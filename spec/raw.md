# RAW File Format

> **Extension:** `.raw` | **Type:** Container | **Contents:** ASCII header + binary event stream

The RAW file is the primary recording format for Prophesee event cameras. It
stores the raw sensor output without decoding or processing, wrapped with an
ASCII metadata header. The binary event data section uses one of the event
stream encodings ([EVT 2.0](evt2.md), [EVT 2.1](evt21.md), or
[EVT 3.0](evt3.md)).

**References:**
- [Official docs](https://docs.prophesee.ai/stable/data/file_formats/raw.html)
- OpenEB: `openeb/hal/cpp/include/metavision/hal/utils/raw_file_header.h`
- OpenEB: `openeb/sdk/modules/base/cpp/src/generic_header.cpp`

## File Structure

```
+----------------------------------+
|         ASCII Header             |  Variable length
|  (% key value\n lines)           |
|  terminated by "% end\n"        |
+----------------------------------+
|                                  |
|      Binary Event Data           |  Remainder of file
|   (EVT 2.0, 2.1, or 3.0)       |
|                                  |
+----------------------------------+
```

The header occupies a variable number of bytes at the beginning of the file.
The binary event data immediately follows and extends to the end of the file.

## Header Format

The header is a sequence of ASCII text lines, each containing a key-value pair.
Every header line has this structure:

```
% <key> <value>\n
```

Specifically:
- Starts with `%` (0x25) followed by a space ` ` (0x20)
- Then a keyword (no spaces)
- Then a space (0x20)
- Then a value (may contain spaces)
- Terminated by newline LF (0x0A)

### Header Termination

The header ends with a special line containing only the keyword `end`:

```
% end\n
```

This `end` marker is **optional**. The parser also detects the end of the
header when it encounters a line that does not begin with `% ` (the byte
sequence 0x25 0x20). In practice, all files written by the Metavision SDK
include the `% end` marker.

### Parsing Algorithm

From the OpenEB reference implementation (`GenericHeader::parse_header`):

```
fn parse_header(stream):
    loop:
        c = stream.peek()
        if c != '%':
            break  // not a header line → data starts here

        stream.read()  // consume '%'
        c = stream.peek()
        if c != ' ':
            stream.unread()  // put '%' back
            break  // not a valid header line

        stream.read()  // consume ' '
        line = stream.read_line()
        (key, value) = split_first_whitespace(line)

        if key == "end":
            break  // explicit header termination

        header[key] = value
```

**Key implementation detail:** The parser peeks two bytes (`%` then ` `)
before committing. If the byte after `%` is not a space, the `%` is put back
and the parser stops — the file position is left at the start of the binary
data.

## Header Fields

### Example Header

From an EVK4 camera recording using EVT 3.0:

```
% camera_integrator_name Prophesee
% date 2023-03-29 16:37:46
% evt 3.0
% format EVT3;height=720;width=1280
% generation 4.2
% geometry 1280x720
% integrator_name Prophesee
% plugin_integrator_name Prophesee
% plugin_name hal_plugin_imx636_evk4
% sensor_generation 4.2
% serial_number 00ca0009
% system_ID 49
% end
```

### Field Reference

| Keyword | Required | Value | Description |
|---------|----------|-------|-------------|
| `format` | **Yes** | `EVTn;height=Y;width=X` | Encoding format + sensor dimensions. This is the primary field for decoding. |
| `geometry` | No | `WxH` (e.g., `1280x720`) | Sensor resolution (redundant with `format`). |
| `camera_integrator_name` | No | string | Company name of the camera manufacturer. |
| `plugin_integrator_name` | No | string | Company name of the HAL plugin provider. |
| `plugin_name` | No | string | HAL plugin used to generate the file. |
| `serial_number` | No | string | Camera serial number. |
| `system_ID` | No | integer | Camera system identifier. Used to infer missing metadata when integrator is `Prophesee`. |
| `date` | No | `YYYY-MM-DD HH:MM:SS` | Recording timestamp. |
| `evt` | No | `2.0`, `2.1`, `3.0` | Encoding format version (legacy, superseded by `format`). |
| `generation` | No | string | Sensor generation (e.g., `4.2`). |
| `sensor_generation` | No | string | Same as `generation` (synonym for compatibility). |
| `integrator_name` | No | string | Same as `camera_integrator_name` (legacy synonym). |
| `end` | Special | *(no value)* | Marks end of header. Not stored as a key-value pair. |

### Format Field Syntax

The `format` field is the most important header field. Its value encodes both
the stream format and the sensor geometry:

```
format_value = FORMAT_NAME ";" "height=" HEIGHT ";" "width=" WIDTH
```

Examples:
- `EVT2;height=480;width=640`
- `EVT21;height=720;width=1280`
- `EVT3;height=720;width=1280`

The `FORMAT_NAME` maps to the encoding:

| Format Name | Encoding |
|-------------|----------|
| `EVT2` | [EVT 2.0](evt2.md) |
| `EVT21` | [EVT 2.1](evt21.md) |
| `EVT3` | [EVT 3.0](evt3.md) |

**Note:** Event frame formats (`HISTO3D`, `DIFF3D`) may also appear in the
format field but are out of scope for this project.

### Backward Compatibility

Some older RAW files may lack the `format` field. In that case, the encoding
can sometimes be inferred from:
1. The `evt` field (e.g., `% evt 3.0` → EVT 3.0)
2. The `system_ID` field combined with `camera_integrator_name = Prophesee`
3. External tools like `metavision_file_info`

The OpenEB `RawFileHeader` class (which extends `GenericHeader`) provides
accessor methods for the critical fields:
- `get_camera_integrator_name()` / `set_camera_integrator_name()`
- `get_plugin_integrator_name()` / `set_plugin_integrator_name()`
- `get_plugin_name()` / `set_plugin_name()` / `remove_plugin_name()`

## Binary Event Data

Immediately after the header (either after the `% end\n` line or after the
last `% ...` line), the remainder of the file contains raw binary event data.

The binary data is a contiguous stream of events encoded in the format
specified by the `format` header field:

| Format | Word Size | Encoding Spec |
|--------|-----------|---------------|
| EVT2 | 32 bits | [EVT 2.0](evt2.md) |
| EVT21 | 64 bits | [EVT 2.1](evt21.md) |
| EVT3 | 16 bits | [EVT 3.0](evt3.md) |

### Byte Order

Little-endian by default for all current sensors (IMX636, GenX320). The byte
order is determined by sensor configuration and applies to the binary data
only — the ASCII header is always plain text.

**EVT 2.1 caveat:** For IMX636 recordings, the 64-bit words are transmitted
as two 32-bit halves (upper half first). See [EVT 2.1 — Byte Order](evt21.md#byte-order)
for details.

### Data Alignment

The binary data starts at the exact byte offset where the header parser
stopped. There is **no padding or alignment** between the header and the data
section. The parser's file position after consuming the header is the first
byte of event data.

## Index File

When reading a RAW file using the Metavision SDK, an index sidecar file may be
automatically generated:

```
<filename>.raw.tmp_index
```

### Purpose

The index file maps timestamp positions within the RAW file to byte offsets,
enabling fast seek operations without scanning the entire event stream.

### Behavior

- **First open:** The SDK scans the RAW file and generates the index. This may
  take noticeable time for large files.
- **Subsequent opens:** The existing index file is reused, making open near-
  instantaneous.
- **Deleted index:** The SDK regenerates it on next open.
- **Write permissions:** If the directory containing the RAW file is not
  writable, the SDK logs a warning and proceeds without indexing.

### Disabling Indexing

The index generation can be disabled programmatically:

```cpp
// C++ HAL API
Metavision::RawFileConfig config;
config.build_index_ = false;
auto device = Metavision::DeviceDiscovery::open_raw_file(file_path, config);

// C++ Stream API
Metavision::FileConfigHints config;
config.set("index", false);
auto camera = Metavision::Camera::from_file(file_path, config);
```

## Reading a RAW File (Pseudocode)

```
fn read_raw_file(path) -> (Header, EventStream):
    stream = open(path, binary_read)

    // Phase 1: Parse header
    header = {}
    loop:
        if stream.peek() != 0x25:  // '%'
            break
        stream.read()  // consume '%'
        if stream.peek() != 0x20:  // ' '
            stream.unread()
            break
        stream.read()  // consume ' '

        line = stream.read_until('\n')
        (key, value) = split_first_space(line)

        if key == "end":
            break
        header[key] = value

    // Phase 2: Determine encoding
    format = parse_format_field(header["format"])
    // format.encoding = "EVT2" | "EVT21" | "EVT3"
    // format.width    = sensor width
    // format.height   = sensor height

    // Phase 3: Read binary data
    data_offset = stream.position()
    event_bytes = stream.read_to_end()

    // Phase 4: Decode using appropriate decoder
    decoder = match format.encoding:
        "EVT2"  -> Evt2Decoder::new()
        "EVT21" -> Evt21Decoder::new()
        "EVT3"  -> Evt3Decoder::new()

    events = decoder.decode(event_bytes)
    return (header, events)
```

## Comparison with HDF5

| Aspect | RAW | HDF5 |
|--------|-----|------|
| Header | ASCII key-value | HDF5 attributes |
| Event storage | Raw encoded stream | Decoded events (x, y, p, t) |
| Compression | None (raw sensor output) | ECF codec (lossless) |
| Seeking | Requires index file | Built-in HDF5 indexing |
| File size | Larger (uncompressed) | Smaller (ECF compressed) |
| Decoding needed | Yes (EVT 2.x / 3.0) | No (already decoded) |
| Third-party access | Custom parser needed | Standard HDF5 tools |
| Best for | Recording, low latency | Archival, analysis |
