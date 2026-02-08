# EVT 2.0 — Event Stream Data Format

> **Word size:** 32 bits | **Vectorized:** No | **Max resolution:** 2048 x 2048

EVT 2.0 is the simplest Prophesee event stream encoding. Each event is a single
32-bit word. It is designed for low event-rate applications.

**References:**
- [Official docs](https://docs.prophesee.ai/stable/data/encoding_formats/evt2.html)
- OpenEB: `openeb/hal/cpp/include/metavision/hal/decoders/evt2/evt2_event_types.h`

## Byte Order

Little-endian by default (IMX636, GenX320). The sensor configuration determines
endianness.

## Word Structure

Every 32-bit word uses bits `[31:28]` (the 4 MSBs) as a **type tag**.

## Event Types

| Type | Name | Value (4-bit) | Description |
|------|------|---------------|-------------|
| CD_OFF | `CD_OFF` | `0b0000` (0x0) | Decrease in illumination (polarity 0) |
| CD_ON | `CD_ON` | `0b0001` (0x1) | Increase in illumination (polarity 1) |
| EVT_TIME_HIGH | `EVT_TIME_HIGH` | `0b1000` (0x8) | Timestamp MSBs |
| EXT_TRIGGER | `EXT_TRIGGER` | `0b1010` (0xA) | External trigger edge |
| IMU_EVT | `IMU_EVT` | `0b1101` (0xD) | IMU data (accelerometer/gyroscope) |
| OTHERS | `OTHERS` | `0b1110` (0xE) | Vendor extensions |
| CONTINUED | `CONTINUED` | `0b1111` (0xF) | Continuation data for multi-word events |

## Timestamp Encoding

Timestamps have **34-bit** precision in microseconds, giving a rollover at
2^34 us = **4 hours 46 minutes**.

The timestamp is split into two parts:

```
Full timestamp (34 bits):
  [33 .............. 6] [5 .. 0]
   EVT_TIME_HIGH (28b)   embedded in CD/Trigger events (6b)
```

- **Low 6 bits** (`timestamp` field): embedded directly in each CD or Trigger
  event word.
- **High 28 bits**: carried by the most recent `EVT_TIME_HIGH` word.

To reconstruct a full timestamp: `full_ts = (last_time_high << 6) | event.timestamp`

Decoding must not produce events until the first `EVT_TIME_HIGH` is received.

## CD_OFF / CD_ON (Change Detection Events)

```
 31       28 27     22 21      11 10       0
+----------+---------+----------+----------+
|   type   |timestamp|    x     |    y     |
|  (4 bit) | (6 bit) | (11 bit) | (11 bit) |
+----------+---------+----------+----------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 31..28 | 4 | `type` | `0b0000` (CD_OFF) or `0b0001` (CD_ON) |
| 27..22 | 6 | `timestamp` | Low 6 bits of microsecond timestamp |
| 21..11 | 11 | `x` | Pixel X coordinate (0..2047) |
| 10..0 | 11 | `y` | Pixel Y coordinate (0..2047) |

Polarity is implicit in the event type: CD_OFF = 0, CD_ON = 1.

**OpenEB C++ struct:**
```c
struct EVT2Event2D {
    unsigned int y : 11;
    unsigned int x : 11;
    unsigned int timestamp : 6;
    unsigned int type : 4;
};
```
(Note: C bitfields pack LSB-first, so `y` occupies bits [10:0], matching the
spec.)

## EVT_TIME_HIGH

```
 31       28 27                             0
+----------+--------------------------------+
|   type   |          timestamp             |
|  (4 bit) |           (28 bit)             |
+----------+--------------------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 31..28 | 4 | `type` | `0b1000` |
| 27..0 | 28 | `timestamp` | Timestamp bits [33:6] |

**OpenEB C++ struct:**
```c
struct EVT2TimeHigh {
    uint32_t ts : 28;
    uint32_t type : 4;
};
```

## EXT_TRIGGER

```
 31       28 27     22 21     13  12     8   7      1    0
+----------+---------+---------+---------+---------+-----+
|   type   |timestamp| unused  |   id    | unused  |value|
|  (4 bit) | (6 bit) | (9 bit) | (5 bit) | (7 bit) |(1b)|
+----------+---------+---------+---------+---------+-----+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 31..28 | 4 | `type` | `0b1010` |
| 27..22 | 6 | `timestamp` | Low 6 bits of timestamp |
| 21..13 | 9 | — | Unused |
| 12..8 | 5 | `id` | Trigger channel: `0x00` = EXTTRIG, `0x01` = TDRSTN/PXRSTN |
| 7..1 | 7 | — | Unused |
| 0 | 1 | `value` | Edge polarity: 0 = falling, 1 = rising |

**OpenEB C++ struct:**
```c
struct EVT2EventExtTrigger {
    unsigned int value : 1;
    unsigned int unused2 : 7;
    unsigned int id : 5;
    unsigned int unused1 : 9;
    unsigned int timestamp : 6;
    unsigned int type : 4;
};
```

## OTHERS (Monitoring / Extensions)

```
 31       28 27     22 21    17  16    15              0
+----------+---------+--------+-----+-----------------+
|   type   |timestamp|unused  |class|    subtype       |
|  (4 bit) | (6 bit) |(5 bit) |(1b) |   (16 bit)      |
+----------+---------+--------+-----+-----------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 31..28 | 4 | `type` | `0b1110` |
| 27..22 | 6 | `timestamp` | Low 6 bits of timestamp |
| 21..17 | 5 | — | Unused |
| 16 | 1 | `class` | 0 = Monitoring, 1 = TBD |
| 15..0 | 16 | `subtype` | Event sub-type (vendor-specific) |

## CONTINUED

```
 31       28 27                             0
+----------+--------------------------------+
|   type   |             data               |
|  (4 bit) |           (28 bit)             |
+----------+--------------------------------+
```

| Bits | Width | Field | Description |
|------|-------|-------|-------------|
| 31..28 | 4 | `type` | `0b1111` |
| 27..0 | 28 | `data` | Interpretation depends on preceding event |

Used for multi-word events (e.g., IMU events use 1 `IMU_EVT` + 5 `CONTINUED`
words to transmit 6-axis accelerometer/gyroscope data).

## IMU Events (Multi-word)

An IMU event spans **6 consecutive 32-bit words**: 1 `IMU_EVT` (type `0xD`) +
5 `CONTINUED` (type `0xF`). Each word carries one 16-bit signed sensor value:

| Word | Type | Payload (bits 16:1) | Bit 0 |
|------|------|---------------------|-------|
| 1 | IMU_EVT (0xD) | `ax` (accel X) | `dmp` flag |
| 2 | CONTINUED (0xF) | `ay` (accel Y) | `dmp` flag |
| 3 | CONTINUED (0xF) | `az` (accel Z) | `dmp` flag |
| 4 | CONTINUED (0xF) | `gx` (gyro X) | `dmp` flag |
| 5 | CONTINUED (0xF) | `gy` (gyro Y) | `dmp` flag |
| 6 | CONTINUED (0xF) | `gz` (gyro Z) | `dmp` flag |

The `dmp` bit indicates whether the Digital Motion Processor is active.
Total: 24 bytes for one complete IMU sample.

## Union Type for Decoding

OpenEB provides a union for decoding any EVT 2.0 word:

```c
union EVT2RawEvent {
    uint32_t raw;           // Raw 32-bit word
    EVT2EvType type;        // Type-only view (for dispatch)
    EVT2Event2D cd;         // CD_OFF or CD_ON
    EVT2TimeHigh th;        // EVT_TIME_HIGH
    EVT2EventExtTrigger trig;  // EXT_TRIGGER
    EVT2EventMonitor monitoring; // OTHERS
};
static_assert(sizeof(EVT2RawEvent) == 4);
```

## Decoding Algorithm (Pseudocode)

```
time_high = 0

for each 32-bit word:
    type = word >> 28

    match type:
        0x0 (CD_OFF):
            ts = (time_high << 6) | ((word >> 22) & 0x3F)
            x  = (word >> 11) & 0x7FF
            y  = word & 0x7FF
            emit CD event (x, y, polarity=0, timestamp=ts)

        0x1 (CD_ON):
            ts = (time_high << 6) | ((word >> 22) & 0x3F)
            x  = (word >> 11) & 0x7FF
            y  = word & 0x7FF
            emit CD event (x, y, polarity=1, timestamp=ts)

        0x8 (EVT_TIME_HIGH):
            time_high = word & 0x0FFFFFFF

        0xA (EXT_TRIGGER):
            ts    = (time_high << 6) | ((word >> 22) & 0x3F)
            id    = (word >> 8) & 0x1F
            value = word & 0x1
            emit trigger event (id, value, timestamp=ts)

        0xE (OTHERS):
            // vendor-specific monitoring
        0xF (CONTINUED):
            // continuation of previous multi-word event
        _:
            // reserved, skip
```
