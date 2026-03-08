# RAW File Format

> `.raw` — ASCII header + binary event stream (EVT 2.0 / 2.1 / 3.0 / 4.0)

## File Layout

```
┌──────────────────────────┐
│     ASCII Header         │  Variable length
│  % key value\n lines     │
│  terminated by % end\n   │
├──────────────────────────┤
│     Binary Event Data    │  Remainder of file
│  (EVT 2.0/2.1/3.0/4.0)  │
└──────────────────────────┘
```

No padding between header and data — binary data starts at the exact byte where
the header parser stopped.

## Header Format

Each line: `% <key> <value>\n` — `%` (0x25), space (0x20), keyword, space, value, LF (0x0A).

Terminated by `% end\n`. The `end` marker is optional; the parser also stops when
it encounters a line not starting with `% ` (0x25 0x20).

### Parsing Algorithm

```
fn parse_header(stream) -> HashMap<String, String>:
    header = {}
    loop:
        if stream.peek() != '%':       break
        stream.read()                   // consume '%'
        if stream.peek() != ' ':
            stream.unread()             // put '%' back
            break
        stream.read()                   // consume ' '
        line = stream.read_line()
        (key, value) = split_first_whitespace(line)
        if key == "end":                break
        header[key] = value
    return header
```

The parser peeks two bytes (`%` then ` `) before committing. File position
after parsing = first byte of binary event data.

### Key Fields

| Field | Required | Example | Description |
|-------|----------|---------|-------------|
| `format` | **Yes** | `EVT3;height=720;width=1280` | Encoding + sensor dimensions — primary decoder selector |
| `geometry` | No | `1280x720` | Sensor resolution (redundant with `format`) |
| `serial_number` | No | `00ca0009` | Camera serial number |
| `date` | No | `2023-03-29 16:37:46` | Recording timestamp |
| `evt` | No | `3.0` | Legacy format version (superseded by `format`) |
| `system_ID` | No | `49` | Camera system ID; used for fallback format inference |
| `generation` | No | `4.2` | Sensor generation |
| `end` | Special | *(none)* | Header terminator, not stored |

### Format Field Syntax

```
FORMAT_NAME;height=HEIGHT;width=WIDTH
```

| Format Name | Encoding |
|-------------|----------|
| `EVT2` | [EVT 2.0](evt20.md) |
| `EVT21` | [EVT 2.1](evt21.md) |
| `EVT3` | [EVT 3.0](evt30.md) |
| `EVT4` | [EVT 4.0](evt40.md) |

### Backward Compatibility

Older files may lack `format`. Fallback: infer from `evt` field (e.g., `% evt 3.0` → EVT 3.0)
or from `system_ID` + `camera_integrator_name = Prophesee`.

## Binary Event Data

Byte order: little-endian for all current sensors.

**EVT 2.1 caveat:** IMX636 transmits 64-bit words as two LE 32-bit halves,
upper first — must swap before interpreting. See [EVT 2.1](evt21.md#byte-order).

## Index Sidecar

The Metavision SDK generates a `<file>.raw.tmp_index` sidecar mapping
timestamps → byte offsets for seek support. Generated on first open, reused on
subsequent opens.
