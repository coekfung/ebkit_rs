# EVT 4.0 — Event Stream Format

> 32-bit words | Vectorized (32 pixels) | Stateless | 34-bit timestamps | Max 2048×2048

Combines EVT 2.0's fixed 32-bit word size with EVT 2.1's 32-pixel vectorization.
Scalar CD events use one word; vectorized CD events use two consecutive words
(header + 32-bit bitmask).

**Note:** Not yet in official Prophesee docs. Reverse-engineered from OpenEB 5.2.0.

## Event Types

| Type tag `[31:28]` | Name | Code | Description |
|--------------------|------|------|-------------|
| `OTHER` | Monitoring/extensions | 0x6 | |
| `CONTINUED` | Continuation for OTHER | 0x7 | |
| `EXT_TRIGGER` | External trigger | 0x9 | |
| `CD_OFF` | Scalar CD decrease (pol 0) | 0xA | |
| `CD_ON` | Scalar CD increase (pol 1) | 0xB | |
| `CD_VEC_OFF` | Vector CD header (pol 0) | 0xC | Two-word event |
| `CD_VEC_ON` | Vector CD header (pol 1) | 0xD | Two-word event |
| `EVT_TIME_HIGH` | Timestamp MSBs | 0xE | |
| `PADDING` | All-bits-set filler | 0xF | `0xFFFFFFFF` |

**Key difference from EVT 2.0:** Type codes are completely different
(CD: 0xA/0xB vs 0x0/0x1, TIME_HIGH: 0xE vs 0x8).

## Timestamps

Same as EVT 2.0/2.1: 34-bit µs = 28-bit high + 6-bit low.
Rollover at 2³⁴ µs ≈ **4h46m**.

```
base_time = last_time_high << 6
full_ts   = base_time + event.timestamp
```

### Loop Detection

Extends timestamps beyond 34 bits via loop counter:
```
MaxTimestamp   = ((1 << 28) - 1) << 6       // 0x3FFFFFFC0
LoopThreshold = (MaxTimestamp >> 1) + 1     // 0x200000000
TimeLoop      = MaxTimestamp + (1 << 6)     // 0x400000000 = 2^34
```
When `base_time >= new_time_high + LoopThreshold`, add `TimeLoop` to shift.

Do not emit events until the first `EVT_TIME_HIGH` is received.

## CD_OFF / CD_ON (Scalar)

```
 31:28   27:22    21:11    10:0
┌──────┬────────┬────────┬────────┐
│ type │  ts_lo │   x    │   y    │
│ (4b) │  (6b)  │ (11b)  │ (11b)  │
└──────┴────────┴────────┴────────┘
```
Type 0xA=CD_OFF (pol 0), 0xB=CD_ON (pol 1). `polarity = type & 1`.

## CD_VEC_OFF / CD_VEC_ON (Vectorized, two words)

**Word 1** — header (same layout as scalar CD):
```
 31:28   27:22    21:11    10:0
┌──────┬────────┬────────┬────────┐
│ type │  ts_lo │   x    │   y    │
│ (4b) │  (6b)  │ (11b)  │ (11b)  │
└──────┴────────┴────────┴────────┘
```
Type 0xC=CD_VEC_OFF, 0xD=CD_VEC_ON.

**Word 2** — validity bitmask:
```
 31:0
┌──────────────────────────────────┐
│             valid (32b)          │
└──────────────────────────────────┘
```
Bit *n* set → event at `(x+n, y)`. May split across buffer boundaries — decoder
must track pending vector state.

## EVT_TIME_HIGH

```
 31:28   27:0
┌──────┬──────────────────────────┐
│ 0xE  │     timestamp[33:6]     │
│ (4b) │         (28b)           │
└──────┴──────────────────────────┘
```

## EXT_TRIGGER

```
 31:28   27:22   21:13   12:8    7:1    0
┌──────┬────────┬───────┬──────┬──────┬─────┐
│ 0x9  │  ts_lo │unused │  id  │count │value│
│ (4b) │  (6b)  │ (9b)  │ (5b) │ (7b) │(1b) │
└──────┴────────┴───────┴──────┴──────┴─────┘
```
`count` field is new vs EVT 2.0 (which has unused bits there).

## OTHER (0x6) + CONTINUED (0x7)

```
OTHER:      [31:28]=0x6  [27:22]=ts_lo  [21:16]=reserved  [15:0]=subtype
CONTINUED:  [31:28]=0x7  [27:0]=data
```

Known sub-types:
- `0x0014` — `MASTER_IN_CD_EVENT_COUNT` (ERC counter)
- `0x0016` — `MASTER_RATE_CONTROL_CD_EVENT_COUNT` (ERC counter)

When preceding OTHER is an ERC sub-type, CONTINUED carries: `[21:0]` = 22-bit
event count. Decoder resets active sub-type after processing.

## Decoding

```
base_time = 0; full_shift = 0; shift_set = false

// Phase 1: Find first EVT_TIME_HIGH
for each u32 word:
    type = word >> 28
    if type == 0xC or type == 0xD: skip next word  // vector mask
    if type == 0xE:
        t = (word & 0x0FFFFFFF) << 6
        if not shift_set: full_shift = -t if time_shifting else 0; shift_set = true
        base_time = t + full_shift
        break to Phase 2

// Phase 2: Decode events
for each u32 word:
    type = word >> 28
    match type:
        0xA | 0xB:  // CD_OFF / CD_ON
            y  = word & 0x7FF
            x  = (word >> 11) & 0x7FF
            ts = base_time + ((word >> 22) & 0x3F)
            emit CD(x, y, pol=type & 1, ts)

        0xC | 0xD:  // CD_VEC_OFF / CD_VEC_ON
            y  = word & 0x7FF
            x  = (word >> 11) & 0x7FF
            ts = base_time + ((word >> 22) & 0x3F)
            mask = next_word()
            while mask != 0:
                offset = ctz(mask)
                mask &= mask - 1
                emit CD(x + offset, y, pol=type & 1, ts)

        0xE:  // EVT_TIME_HIGH
            new_th = ((word & 0x0FFFFFFF) << 6) + full_shift
            if base_time >= new_th + LoopThreshold:
                full_shift += TimeLoop; new_th += TimeLoop
            base_time = new_th

        0x9:  // EXT_TRIGGER
            ts    = base_time + ((word >> 22) & 0x3F)
            id    = (word >> 8) & 0x1F
            value = word & 1
            emit Trigger(id, value, ts)

        0x6:  // OTHER — save subtype for CONTINUED
        0x7:  // CONTINUED — ERC counter if applicable
        0xF:  // PADDING — skip
        _:    // unknown, skip
```
