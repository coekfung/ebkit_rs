# EVT 2.1 — Event Stream Format

> 64-bit words | Vectorized (32 pixels) | 34-bit timestamps | Max 2048×2048

EVT 2.1 extends EVT 2.0 to 64-bit words. Events of the same polarity are
grouped into 32-pixel vectors via a validity bitmask.

## Byte Order

Little-endian, but 64-bit word transmission differs by sensor:

| Sensor | Transmission | Rust approach |
|--------|-------------|---------------|
| **GenX320** | Standard LE 64-bit | `u64::from_le_bytes()` |
| **IMX636** | Two LE 32-bit halves, **upper first** | Read as two `u32` LE, compose `(upper << 32) \| lower` |

For IMX636, the type tag ends up at bits `[31:28]` of the first transmitted
32-bit word rather than `[63:60]`.

## Event Types

| Type tag `[63:60]` | Name | Code |
|--------------------|------|------|
| `EVT_NEG` | CD OFF (pol 0) | 0x0 |
| `EVT_POS` | CD ON (pol 1) | 0x1 |
| `EVT_TIME_HIGH` | Timestamp MSBs | 0x8 |
| `EXT_TRIGGER` | External trigger | 0xA |
| `OTHERS` | Vendor extensions | 0xE |
| `CONTINUED` | Continuation | 0xF |

## Timestamps

Identical to EVT 2.0: 34-bit µs = 28-bit high + 6-bit low.
Rollover at 2³⁴ µs ≈ **4h46m**.

## EVT_NEG / EVT_POS (Vectorized CD)

```
 63:60   59:54    53:43    42:32    31:0
┌──────┬────────┬────────┬────────┬──────────────────┐
│ type │  ts_lo │   x    │   y    │      valid       │
│ (4b) │  (6b)  │ (11b)  │ (11b)  │      (32b)       │
└──────┴────────┴────────┴────────┴──────────────────┘
```

- `x` is aligned on 32 (always a multiple of 32)
- `valid`: bit *n* set → event at `(x+n, y)` with given polarity
- Up to 32 events per word

## EVT_TIME_HIGH

```
 63:60   59:32         31:0
┌──────┬──────────────┬──────────────────┐
│ 0x8  │ ts[33:6]     │    unused (0)    │
│ (4b) │   (28b)      │      (32b)       │
└──────┴──────────────┴──────────────────┘
```

## EXT_TRIGGER

```
 63:60  59:54  53:45  44:40  39:33   32   31:0
┌──────┬──────┬──────┬─────┬──────┬─────┬────────────┐
│ 0xA  │ts_lo │unused│ id  │unused│value│  unused(0) │
│ (4b) │ (6b) │ (9b) │(5b) │ (7b) │(1b) │   (32b)    │
└──────┴──────┴──────┴─────┴──────┴─────┴────────────┘
```

## Decoding

```
time_high = 0

for each u64 word:
    type = word >> 60
    match type:
        0x0 | 0x1:  // EVT_NEG / EVT_POS
            ts    = (time_high << 6) | ((word >> 54) & 0x3F)
            x     = (word >> 43) & 0x7FF    // aligned on 32
            y     = (word >> 32) & 0x7FF
            valid = word & 0xFFFFFFFF
            for n in 0..31:
                if valid & (1 << n):
                    emit CD(x + n, y, pol=type, ts)

        0x8:  // EVT_TIME_HIGH
            time_high = (word >> 32) & 0x0FFFFFFF

        0xA:  // EXT_TRIGGER
            ts    = (time_high << 6) | ((word >> 54) & 0x3F)
            id    = (word >> 40) & 0x1F
            value = (word >> 32) & 0x1
            emit Trigger(id, value, ts)

        0xE: // OTHERS
        0xF: // CONTINUED
        _:   // reserved, skip
```
