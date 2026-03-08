# HDF5 Event File Format

> `.hdf5` — Decoded events (x, y, p, t) with ECF compression

Stores **decoded** event data in [HDF5](https://www.hdfgroup.org/solutions/hdf5/)
format. Unlike [RAW files](raw.md), no EVT decoding is needed when reading.

## File Structure

```
HDF5 Root
├── [attributes]      Metadata (format, geometry, serial_number, ...)
├── CD/               Change Detection events
│   ├── events        Dataset: compound {x, y, p, t}, ECF-compressed
│   └── indexes       Dataset: compound {id, ts}, with offset attribute
└── EXT_TRIGGER/      External Trigger events
    ├── events        (same schema)
    └── indexes       (same schema)
```

## Root Attributes

Mirror the [RAW header fields](raw.md#key-fields):
`format`, `geometry`, `serial_number`, `date`, `system_ID`, `generation`,
`camera_integrator_name`, `plugin_integrator_name`, `plugin_name`.

## Event Schema

```
H5T_COMPOUND {
    H5T_STD_U16LE "x"   // u16 — pixel X coordinate
    H5T_STD_U16LE "y"   // u16 — pixel Y coordinate
    H5T_STD_I16LE "p"   // i16 — polarity: 0=OFF, 1=ON
    H5T_STD_I64LE "t"   // i64 — timestamp in microseconds (absolute)
}
```

Both `CD/events` and `EXT_TRIGGER/events` use this schema. For triggers:
`x`=channel ID, `y`=0, `p`=edge polarity.

Events are stored in timestamp order. Timestamps are absolute — no high/low
reconstruction needed.

## ECF Compression

- **Filter code:** `0x8ECF` (36559)
- **Lossless** HDF5 filter plugin — transparent compress/decompress
- **Source:** [github.com/prophesee-ai/hdf5_ecf](https://github.com/prophesee-ai/hdf5_ecf)
- **Requires:** `HDF5_PLUGIN_PATH` pointing to directory containing the ECF shared library

## Index Dataset

Each group has an `indexes` dataset for timestamp-based seeking.

```
H5T_COMPOUND {
    H5T_STD_I64LE "id"   // i64 — row index in events dataset
    H5T_STD_I64LE "ts"   // i64 — timestamp (offset-adjusted)
}
```

**Interval:** One entry every **2000 µs**.

### Offset Attribute

The `indexes` dataset has an `offset` attribute (typically negative) that
adjusts stored timestamps:

```
stored_ts             = actual_timestamp + offset
actual_timestamp      = stored_ts - offset
```

### Seeking

```
fn seek(indexes, offset, target_ts) -> event_row:
    adjusted = target_ts + offset
    idx = binary_search(indexes.ts, largest ≤ adjusted)
    return indexes[idx].id
```
