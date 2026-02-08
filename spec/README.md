# Event Camera Data Format Specification

This directory contains the data format specifications relevant to the `evcam_rs`
project. These specs are derived from the
[Prophesee Metavision SDK documentation](https://docs.prophesee.ai/stable/data/encoding_formats/index.html)
and the [OpenEB](https://github.com/prophesee-ai/openeb) open-source reference
implementation (vendored under `openeb/`).

## Scope

We cover **event stream data formats** and **file container formats** used to
store raw event-camera output. Event frame formats (Histo3D, Diff3D) and the
legacy AER format are explicitly out of scope for this project.

## Document Map

| Document | Description |
|----------|-------------|
| [EVT 2.0](evt2.md) | 32-bit non-vectorized event stream format |
| [EVT 2.1](evt21.md) | 64-bit vectorized event stream format |
| [EVT 3.0](evt3.md) | 16-bit vectorized, stateful event stream format |
| [RAW File](raw.md) | `.raw` container: ASCII header + binary event stream |
| [HDF5 Event File](hdf5.md) | HDF5-based container with ECF-compressed events |

## Hierarchy

```
                        File Formats (containers)
                       /                          \
                .raw file                     .hdf5 file
            (ASCII header +               (HDF5 groups +
             raw byte stream)              ECF-compressed
                    |                       decoded events)
                    |
          Event Stream Encoding
         /         |          \
      EVT 2.0    EVT 2.1    EVT 3.0
      (32-bit)   (64-bit)   (16-bit)
```

- **Event stream formats** define the binary encoding of individual sensor
  events (CD on/off, timestamps, triggers) as they come off the sensor or
  are stored in `.raw` files.
- **File formats** wrap an event stream with metadata (sensor type, geometry,
  recording date, etc.) to form a self-describing recording.

## Quick Reference: Event Types Across Formats

| Logical Event | EVT 2.0 | EVT 2.1 | EVT 3.0 |
|---------------|---------|---------|---------|
| CD OFF (polarity 0) | `CD_OFF` (0x0) | `EVT_NEG` (0x0) | `EVT_ADDR_Y` + `EVT_ADDR_X` (pol=0) |
| CD ON (polarity 1) | `CD_ON` (0x1) | `EVT_POS` (0x1) | `EVT_ADDR_Y` + `EVT_ADDR_X` (pol=1) |
| Timestamp high | `EVT_TIME_HIGH` (0x8) | `EVT_TIME_HIGH` (0x8) | `EVT_TIME_HIGH` (0x8) |
| Timestamp low | embedded 6-bit | embedded 6-bit | `EVT_TIME_LOW` (0x6) |
| External trigger | `EXT_TRIGGER` (0xA) | `EXT_TRIGGER` (0xA) | `EXT_TRIGGER` (0xA) |
| Vectorized CD | N/A | 32-bit valid mask | `VECT_BASE_X` + `VECT_12`/`VECT_8` |
| Extension | `OTHERS` (0xE) | `OTHERS` (0xE) | `OTHERS` (0xE) |
| Continuation | `CONTINUED` (0xF) | `CONTINUED` (0xF) | `CONTINUED_4` (0x7) / `CONTINUED_12` (0xF) |

## Conventions

- All bit layouts are shown **MSB-first** (highest bit on the left).
- Byte order is **little-endian** by default for all current sensors (IMX636,
  GenX320).
- Timestamps are in **microseconds** throughout.
- The OpenEB C++ reference structs use compiler bitfield packing; the specs
  here describe the logical wire format.

## References

- [Prophesee Data Encoding Formats](https://docs.prophesee.ai/stable/data/encoding_formats/index.html)
- [Prophesee File Formats](https://docs.prophesee.ai/stable/data/file_formats/index.html)
- OpenEB source: `openeb/hal/cpp/include/metavision/hal/decoders/`
