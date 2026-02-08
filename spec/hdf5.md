# HDF5 Event File Format

> **Extension:** `.hdf5` | **Type:** Container | **Compression:** ECF (lossless) | **Standard:** HDF5

HDF5 event files store **decoded** event data (x, y, polarity, timestamp) in the
standard [HDF5](https://www.hdfgroup.org/solutions/hdf5/) hierarchical file
format, compressed with Prophesee's lossless ECF (Event Compression Format)
codec. Unlike [RAW files](raw.md), which contain raw encoded sensor output,
HDF5 files contain fully decoded events — no EVT 2.x/3.0 decoding is needed
when reading.

**References:**
- [Official docs](https://docs.prophesee.ai/stable/data/file_formats/hdf5.html)
- [ECF codec repository](https://github.com/prophesee-ai/hdf5_ecf)

## File Structure

```
HDF5 Root
├── [attributes]           Metadata (format, geometry, serial_number, ...)
├── CD/                    Group: Change Detection events
│   ├── events             Dataset: (x, y, p, t) compound, ECF-compressed
│   └── indexes            Dataset: (id, ts) compound, with offset attribute
└── EXT_TRIGGER/           Group: External Trigger events
    ├── events             Dataset: (x, y, p, t) compound, ECF-compressed
    └── indexes            Dataset: (id, ts) compound, with offset attribute
```

## Root Attributes (Metadata)

The HDF5 file carries metadata as root-level attributes, analogous to the
[RAW file header](raw.md#header-fields). These are standard HDF5 string
attributes.

| Attribute | Description |
|-----------|-------------|
| `format` | Encoding format + dimensions (e.g., `EVT3;height=720;width=1280`) |
| `geometry` | Sensor resolution (e.g., `640x480`) |
| `camera_integrator_name` | Camera manufacturer |
| `plugin_integrator_name` | HAL plugin provider |
| `plugin_name` | HAL plugin used for recording |
| `serial_number` | Camera serial number |
| `system_ID` | Camera system identifier |
| `date` | Recording date |
| `generation` | Sensor generation |

These attributes mirror the RAW header key-value pairs. The `geometry`
attribute is particularly useful for quickly determining the sensor resolution
without reading any event data.

## Event Groups

### CD Group — Change Detection Events

The `CD` group contains the primary event data: pixel-level brightness change
detections from the event camera sensor.

### EXT_TRIGGER Group — External Trigger Events

The `EXT_TRIGGER` group contains external trigger edge events (rising/falling
edges on the camera's trigger input).

Both groups have identical internal structure.

## Events Dataset

Each group contains an `events` dataset — a 1-dimensional compound dataset
with an unlimited maximum size.

### CD Event Schema

```
DATATYPE H5T_COMPOUND {
    H5T_STD_U16LE "x";    // X coordinate (uint16, little-endian)
    H5T_STD_U16LE "y";    // Y coordinate (uint16, little-endian)
    H5T_STD_I16LE "p";    // Polarity    (int16,  little-endian)
    H5T_STD_I64LE "t";    // Timestamp   (int64,  little-endian, microseconds)
}
```

| Field | HDF5 Type | Rust Equivalent | Description |
|-------|-----------|-----------------|-------------|
| `x` | `H5T_STD_U16LE` | `u16` | Pixel X coordinate |
| `y` | `H5T_STD_U16LE` | `u16` | Pixel Y coordinate |
| `p` | `H5T_STD_I16LE` | `i16` | Polarity: 0 = CD_OFF, 1 = CD_ON |
| `t` | `H5T_STD_I64LE` | `i64` | Timestamp in microseconds |

**Notes:**
- Polarity is stored as signed `i16` (not unsigned), though only values 0 and
  1 are used for CD events.
- Timestamps are **absolute microseconds** — no reconstruction from high/low
  parts is needed (unlike EVT 2.x/3.0 raw encoding).
- Events are stored in timestamp order.

### EXT_TRIGGER Event Schema

The `EXT_TRIGGER/events` dataset uses the same compound type. For trigger
events, the fields are reused:

| Field | Trigger Meaning |
|-------|-----------------|
| `x` | Trigger channel ID (e.g., 0 = EXTTRIG) |
| `y` | Unused (0) |
| `p` | Edge polarity: 0 = falling, 1 = rising |
| `t` | Timestamp in microseconds |

## ECF Compression

The `events` datasets are compressed using the **ECF (Event Compression
Format)** codec, registered as an HDF5 filter plugin with the self-assigned
filter code **0x8ECF**.

### Key Properties

- **Lossless:** No event data is lost or modified.
- **HDF5 filter plugin:** Transparent compression/decompression via the
  standard HDF5 filter pipeline.
- **Filter code:** `0x8ECF` (36559 decimal).
- **Open source:** Available at
  [github.com/prophesee-ai/hdf5_ecf](https://github.com/prophesee-ai/hdf5_ecf).

### ECF Plugin Setup

To read HDF5 event files, the ECF filter plugin must be discoverable by the
HDF5 library. The `HDF5_PLUGIN_PATH` environment variable must point to the
directory containing the ECF shared library:

```bash
# Ubuntu 22.04
export HDF5_PLUGIN_PATH=/usr/local/hdf5/lib/plugin

# Ubuntu 24.04
export HDF5_PLUGIN_PATH=/usr/local/lib/hdf5/plugin
```

Without the ECF plugin, HDF5 tools will fail to decompress the `events`
datasets.

## Indexes Dataset

Each group contains an `indexes` dataset — a 1-dimensional compound dataset
that provides a time-based index into the `events` dataset.

### Index Schema

```
DATATYPE H5T_COMPOUND {
    H5T_STD_I64LE "id";   // Event index in the events dataset
    H5T_STD_I64LE "ts";   // Timestamp (with offset applied)
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | `i64` | Index (row number) in the `events` dataset |
| `ts` | `i64` | Timestamp value (offset-adjusted, see below) |

### Index Interval

Events are indexed every **2000 microseconds**. Each index entry points to the
first event at or after the indexed timestamp.

### Offset Attribute

The `indexes` dataset carries an HDF5 attribute called **`offset`** (an
integer). This offset is **subtracted from actual event timestamps** before
storing them in the `ts` field of the index.

```
stored_ts = actual_event_timestamp + offset
actual_event_timestamp = stored_ts - offset
```

(Note: `offset` is typically negative, so subtracting it increases the value.)

### Example

Given `offset = -6384` and these index entries:

| `id` | `ts` |
|------|------|
| 0 | -1 |
| 0 | 0 |
| 77012 | 2000 |
| 154954 | 4000 |
| 234081 | 6000 |

The actual timestamps are:

| `id` | `ts` (stored) | Actual timestamp |
|------|---------------|------------------|
| 77012 | 2000 | 2000 − (−6384) = **8384 us** |
| 154954 | 4000 | 4000 − (−6384) = **10384 us** |
| 234081 | 6000 | 6000 − (−6384) = **12384 us** |

### Seeking by Timestamp

To find events near a target timestamp `ts_target`:

1. Compute the offset-adjusted target: `ts_adjusted = ts_target + offset`
2. Binary-search the `indexes` dataset for the closest `ts` ≤ `ts_adjusted`
3. Read from the `events` dataset starting at the corresponding `id`

```
fn seek_to_timestamp(indexes, offset, ts_target) -> event_index:
    ts_adjusted = ts_target + offset
    // Binary search indexes for largest ts <= ts_adjusted
    idx = binary_search(indexes, ts_adjusted)
    return indexes[idx].id
```

## Reading an HDF5 Event File (Pseudocode)

```
fn read_hdf5_events(path) -> Vec<Event>:
    file = hdf5::open(path, READ)

    // Read metadata
    geometry = file.attr("geometry")  // e.g., "640x480"

    // Read CD events
    cd_group = file.group("CD")
    cd_events = cd_group.dataset("events")
    cd_indexes = cd_group.dataset("indexes")
    cd_offset = cd_indexes.attr("offset")

    // Read all events (ECF decompression happens transparently)
    events = cd_events.read_all()  // Vec<{x: u16, y: u16, p: i16, t: i64}>

    return events
```

## Third-Party Access

HDF5 event files can be read with standard HDF5 tools, provided the ECF
plugin is installed.

### h5dump

```bash
# Show geometry attribute
h5dump -a geometry recording.hdf5

# Show first 10 CD events
h5dump -d "/CD/events" -k 10 recording.hdf5
```

### h5py (Python)

```python
import h5py

with h5py.File("recording.hdf5", "r") as f:
    geometry = f.attrs["geometry"]
    cd_events = f["CD"]["events"]

    # Read first 1000 events
    batch = cd_events[:1000]
    print(batch["x"], batch["y"], batch["p"], batch["t"])
```

### Performance Note

Using HDF5 filter plugins (h5py, h5dump) is **slower** than using the
Metavision SDK's native reader. The filter plugin approach is suitable for
prototyping and analysis, not high-performance event processing.

## Comparison with RAW

| Aspect | RAW | HDF5 |
|--------|-----|------|
| Header | ASCII key-value | HDF5 attributes |
| Event storage | Raw encoded stream (EVT 2.x/3.0) | Decoded events (x, y, p, t) |
| Compression | None | ECF codec (lossless) |
| File size | Larger | Smaller |
| Seeking | Requires `.tmp_index` sidecar | Built-in index dataset |
| Decoding needed | Yes | No |
| Third-party access | Custom parser | Standard HDF5 tools + ECF plugin |
| Write support | SDK + custom tools | SDK only (C++) |
| Best for | Recording, real-time streaming | Archival, offline analysis |
