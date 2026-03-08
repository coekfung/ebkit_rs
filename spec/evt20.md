# EVT 2.0 — Event Stream Format

> 32-bit words | Non-vectorized | 34-bit timestamps | Max 2048×2048

## Event Types

| Type tag `[31:28]` | Name | Code |
|--------------------|------|------|
| `CD_OFF` | CD decrease (pol 0) | 0x0 |
| `CD_ON` | CD increase (pol 1) | 0x1 |
| `EVT_TIME_HIGH` | Timestamp MSBs | 0x8 |
| `EXT_TRIGGER` | External trigger | 0xA |
| `IMU_EVT` | IMU data | 0xD |
| `OTHERS` | Vendor extensions | 0xE |
| `CONTINUED` | Multi-word continuation | 0xF |

## Timestamps

34-bit microseconds, rollover at 2³⁴ µs ≈ **4h46m**.

```
[33:6]  EVT_TIME_HIGH (28 bits)
[5:0]   embedded in each CD/trigger word (6 bits)

full_ts = (last_time_high << 6) | event.timestamp
```

Do not emit events until the first `EVT_TIME_HIGH` is received.

## CD_OFF / CD_ON

```
 31:28   27:22    21:11    10:0
┌──────┬────────┬────────┬────────┐
│ type │  ts_lo │   x    │   y    │
│ (4b) │  (6b)  │ (11b)  │ (11b)  │
└──────┴────────┴────────┴────────┘
```

Polarity implicit: CD_OFF=0, CD_ON=1.

## EVT_TIME_HIGH

```
 31:28   27:0
┌──────┬──────────────────────────┐
│ 0x8  │     timestamp[33:6]     │
│ (4b) │         (28b)           │
└──────┴──────────────────────────┘
```

## EXT_TRIGGER

```
 31:28   27:22   21:13   12:8    7:1     0
┌──────┬────────┬───────┬──────┬───────┬─────┐
│ 0xA  │  ts_lo │unused │  id  │unused │value│
│ (4b) │  (6b)  │ (9b)  │ (5b) │ (7b)  │(1b) │
└──────┴────────┴───────┴──────┴───────┴─────┘
```

- `id`: trigger channel (0x00=EXTTRIG, 0x01=TDRSTN/PXRSTN)
- `value`: 0=falling, 1=rising

## OTHERS

```
 31:28   27:22   21:17   16     15:0
┌──────┬────────┬───────┬─────┬──────────┐
│ 0xE  │  ts_lo │unused │class│ subtype  │
│ (4b) │  (6b)  │ (5b)  │(1b) │  (16b)   │
└──────┴────────┴───────┴─────┴──────────┘
```

## CONTINUED

```
 31:28   27:0
┌──────┬──────────────────────────┐
│ 0xF  │         data            │
│ (4b) │         (28b)           │
└──────┴──────────────────────────┘
```

Interpretation depends on preceding event. IMU uses 1×IMU_EVT + 5×CONTINUED
(6 words = 24 bytes) for 6-axis accelerometer/gyroscope data.

## Decoding

```
time_high = 0

for each u32 word:
    type = word >> 28
    match type:
        0x0 | 0x1:  // CD_OFF / CD_ON
            ts = (time_high << 6) | ((word >> 22) & 0x3F)
            x  = (word >> 11) & 0x7FF
            y  = word & 0x7FF
            emit CD(x, y, pol=type, ts)

        0x8:  // EVT_TIME_HIGH
            time_high = word & 0x0FFFFFFF

        0xA:  // EXT_TRIGGER
            ts    = (time_high << 6) | ((word >> 22) & 0x3F)
            id    = (word >> 8) & 0x1F
            value = word & 0x1
            emit Trigger(id, value, ts)

        0xE: // OTHERS — vendor-specific
        0xF: // CONTINUED — multi-word payload
        _:   // reserved, skip
```
