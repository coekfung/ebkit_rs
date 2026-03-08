# EVT 3.0 — Event Stream Format

> 16-bit words | Vectorized (12/8 pixels) | **Stateful** | 24-bit timestamps | Max 2048×2048

Most compact Prophesee format. Uses stateful encoding — coordinates, polarity,
and timestamps are transmitted only when they change. The decoder must maintain
internal state.

## Event Types

| Type tag `[15:12]` | Name | Code | Description |
|--------------------|------|------|-------------|
| `EVT_ADDR_Y` | Y coordinate | 0x0 | Sets Y + system type |
| `EVT_ADDR_X` | Single CD event | 0x2 | X + polarity → emit 1 event |
| `VECT_BASE_X` | Vector base | 0x3 | Sets base X + polarity (no event) |
| `VECT_12` | 12-bit vector | 0x4 | 12 validity bits → up to 12 events |
| `VECT_8` | 8-bit vector | 0x5 | 8 validity bits → up to 8 events |
| `EVT_TIME_LOW` | Timestamp low | 0x6 | Timestamp bits [11:0] |
| `CONTINUED_4` | 4-bit continuation | 0x7 | |
| `EVT_TIME_HIGH` | Timestamp high | 0x8 | Timestamp bits [23:12] |
| `EXT_TRIGGER` | External trigger | 0xA | |
| `OTHERS` | Vendor extensions | 0xE | |
| `CONTINUED_12` | 12-bit continuation | 0xF | |

Types 0x1, 0x9, 0xB, 0xC are reserved.

## Timestamps

24-bit microseconds (not 34-bit like EVT 2.x), split into two 12-bit halves.
Rollover at 2²⁴ µs ≈ **16.78 seconds**.

```
[23:12]  EVT_TIME_HIGH (12 bits, 4096 µs resolution)
[11:0]   EVT_TIME_LOW  (12 bits, 1 µs resolution)

timestamp = (time_high << 12) | time_low
```

`EVT_TIME_HIGH` is repeated every 16 µs (256× per `EVT_TIME_LOW` period) for
resynchronization after data loss.

## Decoder State

| State variable | Width | Set by | Used by |
|---------------|-------|--------|---------|
| `time_high` | 12b | `EVT_TIME_HIGH` | timestamp reconstruction |
| `time_low` | 12b | `EVT_TIME_LOW` | timestamp reconstruction |
| `y` | 11b | `EVT_ADDR_Y` | all CD events |
| `system_type` | 1b | `EVT_ADDR_Y` | master/slave identification |
| `base_x` | 11b | `VECT_BASE_X`, auto-incremented by VECT_12/VECT_8 | vector events |
| `polarity` | 1b | `EVT_ADDR_X` or `VECT_BASE_X` | CD events |

## Word Layouts

### EVT_ADDR_Y (0x0) — sets Y, no event emitted

```
 15:12   11     10:0
┌──────┬──────┬──────────┐
│ 0x0  │ orig │    y     │
│ (4b) │ (1b) │  (11b)   │
└──────┴──────┴──────────┘
```
`orig`: 0=master, 1=slave. Updates `y` and `system_type`.

### EVT_ADDR_X (0x2) — emits 1 CD event

```
 15:12   11     10:0
┌──────┬──────┬──────────┐
│ 0x2  │ pol  │    x     │
│ (4b) │ (1b) │  (11b)   │
└──────┴──────┴──────────┘
```
Emits `CD(x, state.y, pol, current_ts)`.

### VECT_BASE_X (0x3) — sets base X, no event emitted

```
 15:12   11     10:0
┌──────┬──────┬──────────┐
│ 0x3  │ pol  │    x     │
│ (4b) │ (1b) │  (11b)   │
└──────┴──────┴──────────┘
```
Updates `base_x` and `polarity`.

### VECT_12 (0x4) — up to 12 events

```
 15:12   11:0
┌──────┬──────────────┐
│ 0x4  │    valid     │
│ (4b) │    (12b)     │
└──────┴──────────────┘
```
Bit *i* set → emit `CD(base_x+i, y, polarity, ts)`. After: `base_x += 12`.

### VECT_8 (0x5) — up to 8 events

```
 15:12   11:8     7:0
┌──────┬────────┬──────────┐
│ 0x5  │ unused │  valid   │
│ (4b) │  (4b)  │  (8b)    │
└──────┴────────┴──────────┘
```
Bit *i* set → emit `CD(base_x+i, y, polarity, ts)`. After: `base_x += 8`.

### EVT_TIME_LOW (0x6)

```
 15:12   11:0
┌──────┬──────────────┐
│ 0x6  │  time_low    │
│ (4b) │    (12b)     │
└──────┴──────────────┘
```

### EVT_TIME_HIGH (0x8)

```
 15:12   11:0
┌──────┬──────────────┐
│ 0x8  │  time_high   │
│ (4b) │    (12b)     │
└──────┴──────────────┘
```

### EXT_TRIGGER (0xA)

```
 15:12   11:8    7:1     0
┌──────┬──────┬───────┬─────┐
│ 0xA  │  id  │unused │value│
│ (4b) │ (4b) │ (7b)  │(1b) │
└──────┴──────┴───────┴─────┘
```

### CONTINUED_4 (0x7) / CONTINUED_12 (0xF)

```
CONTINUED_4:   [15:12]=0x7  [11:4]=unused  [3:0]=data(4b)
CONTINUED_12:  [15:12]=0xF  [11:0]=data(12b)
```

Payloads concatenate for larger values (e.g., 12+12+4=28 bits).

## Typical Sequence

```
EVT_TIME_HIGH  (set time_high)
EVT_TIME_LOW   (set time_low → full timestamp established)
EVT_ADDR_Y     (y=100)
EVT_ADDR_X     (pol=1, x=50)        → emit (50, 100, ON, ts)
EVT_ADDR_Y     (y=101)
VECT_BASE_X    (pol=1, x=32)        → no event
VECT_12        (valid=0b110000000011) → emit (32,101), (33,101), (42,101), (43,101)
                                       base_x → 44
VECT_8         (valid=0b00000001)    → emit (44, 101)
                                       base_x → 52
```

## Decoding

```
state = { time_high: 0, time_low: 0, y: 0, base_x: 0, polarity: 0, system_type: 0 }
ts() = (state.time_high << 12) | state.time_low

for each u16 word:
    type = word >> 12
    match type:
        0x0:  state.y = word & 0x7FF; state.system_type = (word >> 11) & 1
        0x2:  emit CD(word & 0x7FF, state.y, (word >> 11) & 1, ts())
        0x3:  state.base_x = word & 0x7FF; state.polarity = (word >> 11) & 1
        0x4:  // VECT_12
              valid = word & 0xFFF
              for i in 0..11: if valid & (1<<i): emit CD(state.base_x+i, state.y, state.polarity, ts())
              state.base_x += 12
        0x5:  // VECT_8
              valid = word & 0xFF
              for i in 0..7: if valid & (1<<i): emit CD(state.base_x+i, state.y, state.polarity, ts())
              state.base_x += 8
        0x6:  state.time_low = word & 0xFFF
        0x8:  state.time_high = word & 0xFFF
        0xA:  emit Trigger((word >> 8) & 0xF, word & 1, ts())
        0xE:  // OTHERS — vendor-specific
        0x7:  // CONTINUED_4
        0xF:  // CONTINUED_12
        _:    // reserved, skip
```
