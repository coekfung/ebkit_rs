# Event Camera Data Format Specification

Specs derived from [Prophesee Metavision SDK docs](https://docs.prophesee.ai/stable/data/encoding_formats/index.html)
and [OpenEB](https://github.com/prophesee-ai/openeb) (vendored under `openeb/`).

**Scope:** Event stream encodings + file containers for raw event-camera output.
Out of scope: event frames (Histo3D, Diff3D), legacy AER format.

## Documents

| Document | Description |
|----------|-------------|
| [RAW File](raw.md) | `.raw` container: ASCII header + binary event stream |
| [EVT 2.0](evt20.md) | 32-bit non-vectorized event stream |
| [EVT 2.1](evt21.md) | 64-bit vectorized event stream (32-pixel groups) |
| [EVT 3.0](evt30.md) | 16-bit stateful vectorized event stream |
| [EVT 4.0](evt40.md) | 32-bit vectorized event stream (scalar + vector CD) |
| [HDF5](hdf5.md) | HDF5 container with ECF-compressed decoded events |

## Hierarchy

```
               File Containers
              /               \
       .raw file            .hdf5 file
    (header + raw          (ECF-compressed
     byte stream)           decoded events)
          |
    Event Encoding
    /    |    |    \
 EVT2  EVT2.1 EVT3  EVT4
```

## Format Comparison

| | EVT 2.0 | EVT 2.1 | EVT 3.0 | EVT 4.0 |
|-|---------|---------|---------|---------|
| Word size | 32-bit | 64-bit | 16-bit | 32-bit |
| Vectorized | No | 32px | 12/8px | 32px (scalar+vec) |
| Stateful | No | No | **Yes** | No |
| Timestamp bits | 34 (28+6) | 34 (28+6) | 24 (12+12) | 34 (28+6) |
| Rollover | ~4h46m | ~4h46m | ~16.78s | ~4h46m |
| Max events/word | 1 | 32 | 12 | 1 scalar / 32 vec |

## Event Type Codes

| Logical Event | EVT 2.0 | EVT 2.1 | EVT 3.0 | EVT 4.0 |
|---------------|---------|---------|---------|---------|
| CD OFF | 0x0 | 0x0 | stateful | 0xA |
| CD ON | 0x1 | 0x1 | stateful | 0xB |
| Time high | 0x8 | 0x8 | 0x8 | 0xE |
| Time low | embedded 6-bit | embedded 6-bit | 0x6 (12-bit) | embedded 6-bit |
| Ext trigger | 0xA | 0xA | 0xA | 0x9 |
| Vector CD | N/A | valid mask | VECT_12/VECT_8 | 0xC/0xD + mask |
| Extension | 0xE | 0xE | 0xE | 0x6 |
| Continuation | 0xF | 0xF | 0x7/0xF | 0x7 |
| Padding | N/A | N/A | N/A | 0xF |

## Conventions

- Bit layouts: **MSB-first** (highest bit on left)
- Byte order: **little-endian** (all current sensors: IMX636, GenX320)
- Timestamps: **microseconds** throughout
- OpenEB C++ structs use compiler bitfield packing; specs describe logical wire format
