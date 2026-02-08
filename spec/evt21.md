# EVT 2.1 — Event Stream Data Format

> **Word size:** 64 bits | **Vectorized:** Yes (32 pixels) | **Max resolution:** 2048 x 2048

EVT 2.1 extends EVT 2.0 to 64-bit words with vectorization. Events of the same
polarity are grouped into 32-pixel vectors, making it efficient for high
event-rate applications.

**References:**
- [Official docs](https://docs.prophesee.ai/stable/data/encoding_formats/evt21.html)
- OpenEB: `openeb/hal/cpp/include/metavision/hal/decoders/evt21/evt21_event_types.h`

## Byte Order

Little-endian by default, but the 64-bit word transmission differs by sensor:

- **IMX636:** The 64-bit word is sent as two 32-bit halves, each in
  little-endian. The upper 32-bit half is sent first, then the lower 32-bit
  half.
  - Data `0x0102030405060708` is transmitted as `0x0403020108070605`
- **GenX320:** The entire 64-bit word is sent in little-endian at once.
  - Data `0x0102030405060708` is transmitted as `0x0807060504030201`

**Important for Rust:** When reading from a GenX320 recording, a simple
`u64::from_le_bytes()` suffices. For IMX636 recordings, you must swap the two
32-bit halves before interpreting, or read as two `u32::from_le_bytes()` and
compose `(upper << 32) | lower`.

OpenEB provides two struct layouts for this:
- `Evt21Raw` — standard layout (GenX320): type is at bits [63:60]
- `Evt21LegacyRaw` — IMX636 "swapped halves" layout: type is at bits [31:28]
  of the first transmitted 32-bit word

## Word Structure

Every 64-bit word uses bits `[63:60]` (the 4 MSBs) as a **type tag**.

## Event Types

| Type | Name | Value (4-bit) | Description |
|------|------|---------------|-------------|
| EVT_NEG | `EVT_NEG` | `0b0000` (0x0) | CD OFF — decrease in illumination (polarity 0) |
| EVT_POS | `EVT_POS` | `0b0001` (0x1) | CD ON — increase in illumination (polarity 1) |
| EVT_TIME_HIGH | `EVT_TIME_HIGH` | `0b1000` (0x8) | Timestamp MSBs |
| EXT_TRIGGER | `EXT_TRIGGER` | `0b1010` (0xA) | External trigger edge |
| OTHERS | `OTHERS` | `0b1110` (0xE) | Vendor extensions |
| CONTINUED | `CONTINUED` | `0b1111` (0xF) | Continuation data |

## Timestamp Encoding

Identical scheme to EVT 2.0: **34-bit** microsecond timestamps, split into
28-bit high (in `EVT_TIME_HIGH`) + 6-bit low (embedded in event words).

```
Full timestamp (34 bits):
  [33 .............. 6] [5 .. 0]
   EVT_TIME_HIGH (28b)   embedded in CD/Trigger events (6b)
```

Rollover at 2^34 us = **4 hours 46 minutes**.

## EVT_NEG / EVT_POS (Vectorized CD Events)

```
 63       60 59     54 53      43 42      32 31                      0
+----------+---------+----------+----------+-------------------------+
|   type   |timestamp|    x     |    y     |         valid           |
|  (4 bit) | (6 bit) | (11 bit) | (11 bit) |        (32 bit)        |
+----------+---------+----------+----------+-------------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 63..60 | 4 | `type` | `0x0` (EVT_NEG) or `0x1` (EVT_POS) |
| 59..54 | 6 | `timestamp` | Low 6 bits of microsecond timestamp |
| 53..43 | 11 | `x` | Pixel X coordinate, **aligned on 32** (i.e., always a multiple of 32) |
| 42..32 | 11 | `y` | Pixel Y coordinate |
| 31..0 | 32 | `valid` | Bitmask: bit *n* set means a valid event at (x+n, y) |

This is the key difference from EVT 2.0: a single 64-bit word can encode
up to 32 events of the same polarity, Y coordinate, and timestamp.

**Decoding:** For each set bit *n* in `valid` (0..31), emit a CD event at
coordinates `(x + n, y)` with the given polarity and timestamp.

**OpenEB C++ struct (standard layout):**
```c
struct Event_2D {
    uint64_t valid : 32;  // bits [31:0]
    uint64_t y : 11;      // bits [42:32]
    uint64_t x : 11;      // bits [53:43]
    uint64_t ts : 6;      // bits [59:54]
    uint64_t type : 4;    // bits [63:60]
};
```

**OpenEB C++ struct (IMX636 legacy layout):**
```c
struct Event_2D {
    uint64_t y : 11;      // bits [10:0]   (of first 32-bit word)
    uint64_t x : 11;      // bits [21:11]
    uint64_t ts : 6;      // bits [27:22]
    uint64_t type : 4;    // bits [31:28]
    uint64_t valid : 32;  // bits [63:32]  (second 32-bit word)
};
```

## EVT_TIME_HIGH

```
 63       60 59                 32 31                      0
+----------+---------------------+-------------------------+
|   type   |     timestamp       |        unused           |
|  (4 bit) |      (28 bit)       |       (32 bit = 0)      |
+----------+---------------------+-------------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 63..60 | 4 | `type` | `0x8` |
| 59..32 | 28 | `timestamp` | Timestamp bits [33:6] |
| 31..0 | 32 | — | Unused, stuck at zero |

## EXT_TRIGGER

```
 63    60 59   54 53   45 44  40 39  33  32  31               0
+-------+------+-------+-----+------+----+-------------------+
| type  |  ts  |unused | id  |unused|val |      unused       |
|(4 bit)|(6 b) |(9 b)  |(5b) |(7 b) |(1b)|     (32 bit = 0)  |
+-------+------+-------+-----+------+----+-------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 63..60 | 4 | `type` | `0xA` |
| 59..54 | 6 | `timestamp` | Low 6 bits of timestamp |
| 53..45 | 9 | — | Unused (zero) |
| 44..40 | 5 | `id` | Trigger channel: `0x00` = EXTTRIG, `0x01` = TDRSTN/PXRSTN |
| 39..33 | 7 | — | Unused (zero) |
| 32 | 1 | `value` | Edge polarity: 0 = falling, 1 = rising |
| 31..0 | 32 | — | Unused (zero) |

## Decoding Algorithm (Pseudocode)

```
time_high = 0

for each 64-bit word:
    type = word >> 60

    match type:
        0x0 (EVT_NEG) | 0x1 (EVT_POS):
            ts    = (time_high << 6) | ((word >> 54) & 0x3F)
            x     = (word >> 43) & 0x7FF   // aligned on 32
            y     = (word >> 32) & 0x7FF
            valid = word & 0xFFFFFFFF
            pol   = type  // 0 = OFF, 1 = ON

            for n in 0..31:
                if valid & (1 << n):
                    emit CD event (x + n, y, polarity=pol, timestamp=ts)

        0x8 (EVT_TIME_HIGH):
            time_high = (word >> 32) & 0x0FFFFFFF

        0xA (EXT_TRIGGER):
            ts    = (time_high << 6) | ((word >> 54) & 0x3F)
            id    = (word >> 40) & 0x1F
            value = (word >> 32) & 0x1
            emit trigger event (id, value, timestamp=ts)

        0xE (OTHERS):
            // vendor-specific
        0xF (CONTINUED):
            // continuation of previous event
        _:
            // reserved, skip
```

## Comparison with EVT 2.0

| Aspect | EVT 2.0 | EVT 2.1 |
|--------|---------|---------|
| Word size | 32 bits | 64 bits |
| Events per word | 1 | Up to 32 |
| Vectorization | No | Yes (32-pixel X groups) |
| Timestamp scheme | Same | Same (34-bit split) |
| Best for | Low event rate | High event rate |
| Type tag naming | CD_OFF/CD_ON | EVT_NEG/EVT_POS |
