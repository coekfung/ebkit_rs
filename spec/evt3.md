# EVT 3.0 — Event Stream Data Format

> **Word size:** 16 bits | **Vectorized:** Yes | **Stateful:** Yes | **Max resolution:** 2048 x 2048

EVT 3.0 is the most compact Prophesee format. It uses 16-bit words and a
**stateful encoding** where coordinates, polarity, and timestamps are
transmitted only when they change. The decoder must maintain internal state
to reconstruct the full event stream.

**References:**
- [Official docs](https://docs.prophesee.ai/stable/data/encoding_formats/evt3.html)
- OpenEB: `openeb/hal/cpp/include/metavision/hal/decoders/evt3/evt3_event_types.h`

## Byte Order

Little-endian by default (IMX636, GenX320).

## Word Structure

Every 16-bit word uses bits `[15:12]` (the 4 MSBs) as a **type tag**.

## Event Types

| Type | Name | Value (4-bit) | Description |
|------|------|---------------|-------------|
| 0x0 | `EVT_ADDR_Y` | `0b0000` | Y coordinate + system type (master/slave) |
| 0x2 | `EVT_ADDR_X` | `0b0010` | Single CD event: X coordinate + polarity |
| 0x3 | `VECT_BASE_X` | `0b0011` | Base X + polarity for subsequent vector events |
| 0x4 | `VECT_12` | `0b0100` | Vector: 12 consecutive pixel validity bits |
| 0x5 | `VECT_8` | `0b0101` | Vector: 8 consecutive pixel validity bits |
| 0x6 | `EVT_TIME_LOW` | `0b0110` | Low 12 bits of timestamp |
| 0x7 | `CONTINUED_4` | `0b0111` | 4-bit continuation data |
| 0x8 | `EVT_TIME_HIGH` | `0b1000` | High 12 bits of timestamp |
| 0xA | `EXT_TRIGGER` | `0b1010` | External trigger edge |
| 0xD | `IMU` | `0b1101` | IMU data (reserved) |
| 0xE | `OTHERS` | `0b1110` | Vendor extensions |
| 0xF | `CONTINUED_12` | `0b1111` | 12-bit continuation data |

Types `0x1`, `0x9`, `0xB`, `0xC` are reserved/unused.

## Timestamp Encoding

EVT 3.0 uses a **24-bit** timestamp (not 34-bit like EVT 2.x), split into two
12-bit halves:

```
Full timestamp (24 bits):
  [23 ........... 12] [11 ....... 0]
   EVT_TIME_HIGH (12b) EVT_TIME_LOW (12b)
```

- **`EVT_TIME_LOW`**: 12 bits at 1 us resolution → range 0..4095 us. Wraps
  after 4095 us, at which point `EVT_TIME_HIGH` increments.
- **`EVT_TIME_HIGH`**: 12 bits at 4096 us resolution → combined range
  0..16,777,215 us (~16.78 seconds). Wraps after that.

**Redundancy:** Within each `EVT_TIME_LOW` period, the same `EVT_TIME_HIGH`
value is repeated 256 times (every 16 us) to allow faster resynchronization
after data loss.

**Monotonicity:**
- `EVT_TIME_HIGH` is globally monotonic (across all event sources).
- `EVT_TIME_LOW` is only monotonic within a single event source, but may be
  non-monotonic across sources. All sources within a `EVT_TIME_LOW` period
  share the same `EVT_TIME_HIGH`.

**Reconstruction:**
```
timestamp = (last_time_high << 12) | time_low
```

## Decoder State

The EVT 3.0 decoder must maintain:

| State | Updated by | Used by |
|-------|-----------|---------|
| `time_high` (12 bits) | `EVT_TIME_HIGH` | timestamp reconstruction |
| `time_low` (12 bits) | `EVT_TIME_LOW` | timestamp reconstruction |
| `y` (11 bits) | `EVT_ADDR_Y` | all CD events |
| `system_type` (1 bit) | `EVT_ADDR_Y` | master/slave identification |
| `base_x` (11 bits) | `VECT_BASE_X`, auto-incremented by `VECT_12`/`VECT_8` | vector events |
| `polarity` (1 bit) | `EVT_ADDR_X` or `VECT_BASE_X` | CD events |

## EVT_ADDR_Y

Sets the Y coordinate for all subsequent events until the next `EVT_ADDR_Y`.

```
 15       12  11    10           0
+----------+------+--------------+
|   type   | orig |      y       |
|  (4 bit) |(1 b) |   (11 bit)   |
+----------+------+--------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x0` |
| 11 | 1 | `system_type` | 0 = Master camera, 1 = Slave camera |
| 10..0 | 11 | `y` | Pixel Y coordinate (0..2047) |

**Action:** Update decoder state `y` and `system_type`. No event emitted.

## EVT_ADDR_X (Single Event)

Emits a single CD event at the given X coordinate with the current Y and
timestamp.

```
 15       12  11    10           0
+----------+------+--------------+
|   type   | pol  |      x       |
|  (4 bit) |(1 b) |   (11 bit)   |
+----------+------+--------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x2` |
| 11 | 1 | `pol` | Polarity: 0 = CD_OFF, 1 = CD_ON |
| 10..0 | 11 | `x` | Pixel X coordinate (0..2047) |

**Action:** Emit one CD event `(x, current_y, pol, current_timestamp)`.

## VECT_BASE_X (Vector Base Address)

Sets the base X coordinate and polarity for subsequent `VECT_12` / `VECT_8`
words. **Does not emit an event itself.**

```
 15       12  11    10           0
+----------+------+--------------+
|   type   | pol  |      x       |
|  (4 bit) |(1 b) |   (11 bit)   |
+----------+------+--------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x3` |
| 11 | 1 | `pol` | Polarity: 0 = CD_OFF, 1 = CD_ON |
| 10..0 | 11 | `x` | Base X coordinate |

**Action:** Update decoder state `base_x = x` and `polarity = pol`. No event
emitted.

## VECT_12 (12-bit Vector)

Encodes up to 12 consecutive events relative to the current `base_x`.

```
 15       12 11                  0
+----------+---------------------+
|   type   |       valid         |
|  (4 bit) |      (12 bit)      |
+----------+---------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x4` |
| 11..0 | 12 | `valid` | Bitmask: bit *i* set → event at `(base_x + i, y)` |

**Action:**
1. For each set bit *i* in `valid` (0..11): emit CD event
   `(base_x + i, current_y, current_polarity, current_timestamp)`.
2. After processing: `base_x += 12`.

## VECT_8 (8-bit Vector)

Encodes up to 8 consecutive events relative to the current `base_x`.

```
 15       12 11        8  7             0
+----------+----------+----------------+
|   type   |  unused  |     valid      |
|  (4 bit) | (4 bit)  |    (8 bit)     |
+----------+----------+----------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x5` |
| 11..8 | 4 | — | Unused |
| 7..0 | 8 | `valid` | Bitmask: bit *i* set → event at `(base_x + i, y)` |

**Action:**
1. For each set bit *i* in `valid` (0..7): emit CD event
   `(base_x + i, current_y, current_polarity, current_timestamp)`.
2. After processing: `base_x += 8`.

## EVT_TIME_LOW

```
 15       12 11                  0
+----------+---------------------+
|   type   |     evt_time_low    |
|  (4 bit) |      (12 bit)      |
+----------+---------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x6` |
| 11..0 | 12 | `evt_time_low` | Timestamp bits [11:0] (1 us resolution) |

**Action:** Update `time_low`. Full timestamp = `(time_high << 12) | time_low`.

## EVT_TIME_HIGH

```
 15       12 11                  0
+----------+---------------------+
|   type   |     evt_time_high   |
|  (4 bit) |      (12 bit)      |
+----------+---------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x8` |
| 11..0 | 12 | `evt_time_high` | Timestamp bits [23:12] (4096 us resolution) |

**Action:** Update `time_high`. Specifically:
```
cur_t = (cur_t & ~(0xFFF << 12)) | (evt_time_high << 12)
```

This preserves the current `time_low` and only replaces bits [23:12].

## EXT_TRIGGER

```
 15       12 11        8  7         1    0
+----------+----------+-----------+-----+
|   type   |    id    |  unused   |value|
|  (4 bit) | (4 bit)  |  (7 bit)  |(1b) |
+----------+----------+-----------+-----+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0xA` |
| 11..8 | 4 | `id` | Trigger channel: `0x0` = EXTTRIG, `0x1` = TDRSTN/PXRSTN |
| 7..1 | 7 | — | Unused |
| 0 | 1 | `value` | Edge polarity: 0 = falling, 1 = rising |

## OTHERS

```
 15       12 11                  0
+----------+---------------------+
|   type   |       subtype       |
|  (4 bit) |      (12 bit)      |
+----------+---------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0xE` |
| 11..0 | 12 | `subtype` | Vendor-specific sub-type |

Additional payload may follow in `CONTINUED_4` or `CONTINUED_12` words.

## CONTINUED_4

```
 15       12 11        4  3       0
+----------+----------+----------+
|   type   |  unused  |   data   |
|  (4 bit) | (8 bit)  | (4 bit)  |
+----------+----------+----------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0x7` |
| 11..4 | 8 | — | Unused |
| 3..0 | 4 | `data` | Interpretation depends on preceding event |

## CONTINUED_12

```
 15       12 11                  0
+----------+---------------------+
|   type   |        data         |
|  (4 bit) |      (12 bit)      |
+----------+---------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 15..12 | 4 | `type` | `0xF` |
| 11..0 | 12 | `data` | Interpretation depends on preceding event |

When multiple continuation words follow, their payloads are concatenated to
form larger values. OpenEB uses sequences of `CONTINUED_12 + CONTINUED_12 +
CONTINUED_4` to encode 28-bit values (12 + 12 + 4 = 28 bits).

## Typical Encoding Sequence

A typical burst of CD events in EVT 3.0 looks like:

```
EVT_TIME_HIGH  (set upper timestamp)
EVT_TIME_LOW   (set lower timestamp → establishes full timestamp)
EVT_ADDR_Y     (set Y = 100)
EVT_ADDR_X     (pol=1, x=50)        → emit (50, 100, ON, ts)
EVT_ADDR_Y     (set Y = 101)
VECT_BASE_X    (pol=1, x=32)        → (no event emitted)
VECT_12        (valid=0b110000000011) → emit (32,101,ON,ts), (33,101,ON,ts),
                                             (42,101,ON,ts), (43,101,ON,ts)
                                        base_x advances to 44
VECT_8         (valid=0b00000001)    → emit (44, 101, ON, ts)
                                        base_x advances to 52
EVT_TIME_LOW   (advance timestamp)
...
```

## Decoding Algorithm (Pseudocode)

```
state = {
    time_high: 0,
    time_low: 0,
    y: 0,
    base_x: 0,
    polarity: 0,
    system_type: 0,  // master
}

fn current_timestamp():
    return (state.time_high << 12) | state.time_low

for each 16-bit word:
    type = word >> 12

    match type:
        0x0 (EVT_ADDR_Y):
            state.y = word & 0x7FF
            state.system_type = (word >> 11) & 0x1

        0x2 (EVT_ADDR_X):
            x   = word & 0x7FF
            pol = (word >> 11) & 0x1
            emit CD event (x, state.y, pol, current_timestamp())

        0x3 (VECT_BASE_X):
            state.base_x  = word & 0x7FF
            state.polarity = (word >> 11) & 0x1
            // no event emitted

        0x4 (VECT_12):
            valid = word & 0xFFF
            for i in 0..11:
                if valid & (1 << i):
                    emit CD event (state.base_x + i, state.y,
                                   state.polarity, current_timestamp())
            state.base_x += 12

        0x5 (VECT_8):
            valid = word & 0xFF
            for i in 0..7:
                if valid & (1 << i):
                    emit CD event (state.base_x + i, state.y,
                                   state.polarity, current_timestamp())
            state.base_x += 8

        0x6 (EVT_TIME_LOW):
            state.time_low = word & 0xFFF

        0x8 (EVT_TIME_HIGH):
            state.time_high = word & 0xFFF

        0xA (EXT_TRIGGER):
            id    = (word >> 8) & 0xF
            value = word & 0x1
            emit trigger event (id, value, current_timestamp())

        0xE (OTHERS):
            subtype = word & 0xFFF
            // handle vendor-specific event
        0x7 (CONTINUED_4):
            data = word & 0xF
            // append to previous event
        0xF (CONTINUED_12):
            data = word & 0xFFF
            // append to previous event
        _:
            // reserved, skip
```

## Comparison with EVT 2.x

| Aspect | EVT 2.0 | EVT 2.1 | EVT 3.0 |
|--------|---------|---------|---------|
| Word size | 32 bits | 64 bits | **16 bits** |
| Stateful | No | No | **Yes** |
| Timestamp bits | 34 (28+6) | 34 (28+6) | **24** (12+12) |
| Timestamp rollover | ~4h46m | ~4h46m | **~16.78s** |
| Max events/word | 1 | 32 | 12 (VECT_12) |
| Compression | None | Vectorized | **Highly compressed** |
| Decoding complexity | Simple | Simple | **Stateful** |
| Best for | Low rate | High rate | **High rate, bandwidth-constrained** |
