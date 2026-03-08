//! RAW file header parser — see `spec/raw.md`.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::str;

use ebkit_core::{EventEncoding, Geometry, Metadata};
use thiserror::Error;
use winnow::combinator::{repeat, terminated};
use winnow::token::{literal, take_till};
use winnow::{ModalResult, Parser};

#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("missing `format` (and `evt`) header field — cannot determine encoding")]
    MissingEncoding,

    #[error("unknown format name `{0}` (expected EVT2, EVT21, or EVT3)")]
    UnknownFormat(String),

    #[error("invalid numeric value for `{field}`: `{value}`")]
    InvalidNumber { field: String, value: String },

    #[error("malformed `format` field: `{0}`")]
    MalformedFormat(String),

    #[error("malformed `geometry` field: `{0}` (expected WxH)")]
    MalformedGeometry(String),

    #[error("missing geometry — no `format` or `geometry` header field")]
    MissingGeometry,
}

/// Parsed RAW file header — a collection of `% key value` pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headers(BTreeMap<String, String>);

impl Headers {
    /// # Field mapping
    ///
    /// | Header key                 | `Metadata` field              |
    /// |----------------------------|-------------------------------|
    /// | `format`                   | `encoding` + `geometry`       |
    /// | `geometry`                 | `geometry` (fallback)         |
    /// | `evt`                      | `encoding` (fallback)         |
    /// | `camera_integrator_name`   | `camera_integrator_name`      |
    /// | `integrator_name`          | `camera_integrator_name` (legacy synonym) |
    /// | `plugin_integrator_name`   | `plugin_integrator_name`      |
    /// | `plugin_name`              | `plugin_name`                 |
    /// | `serial_number`            | `serial_number`               |
    /// | `system_ID`                | `system_id`                   |
    /// | `date`                     | `date`                        |
    /// | `generation`               | `generation`                  |
    /// | `sensor_generation`        | `generation` (synonym)        |
    /// | *(anything else)*          | `extra`                       |
    pub fn into_metadata(self) -> Result<Metadata, MetadataError> {
        let mut headers = self.0;

        let mut encoding = None;
        let mut geometry = None;

        if let Some(format_val) = headers.remove("format") {
            let (enc, geom) = parse_format_field(&format_val)?;
            encoding = Some(enc);
            geometry = Some(geom);
        }

        if let Some(evt_val) = headers.remove("evt") {
            if encoding.is_none() {
                encoding = Some(encoding_from_evt_field(&evt_val)?);
            }
        }

        if let Some(geom_val) = headers.remove("geometry") {
            if geometry.is_none() {
                geometry = Some(parse_geometry_field(&geom_val)?);
            }
        }

        if encoding.is_none() {
            return Err(MetadataError::MissingEncoding);
        }

        let geometry = geometry.ok_or(MetadataError::MissingGeometry)?;

        let mut meta = Metadata {
            encoding,
            geometry,
            camera_integrator_name: None,
            plugin_integrator_name: None,
            plugin_name: None,
            serial_number: None,
            system_id: None,
            date: None,
            generation: None,
            extra: BTreeMap::new(),
        };

        if let Some(v) = headers.remove("camera_integrator_name") {
            meta.camera_integrator_name = Some(v);
        }
        if let Some(v) = headers.remove("integrator_name") {
            if meta.camera_integrator_name.is_none() {
                meta.camera_integrator_name = Some(v);
            } else {
                meta.extra.insert("integrator_name".to_owned(), v);
            }
        }

        if let Some(v) = headers.remove("plugin_integrator_name") {
            meta.plugin_integrator_name = Some(v);
        }
        if let Some(v) = headers.remove("plugin_name") {
            meta.plugin_name = Some(v);
        }
        if let Some(v) = headers.remove("serial_number") {
            meta.serial_number = Some(v);
        }
        if let Some(v) = headers.remove("system_ID") {
            meta.system_id = Some(v);
        }
        if let Some(v) = headers.remove("date") {
            meta.date = Some(v);
        }

        if let Some(v) = headers.remove("generation") {
            meta.generation = Some(v);
        }
        if let Some(v) = headers.remove("sensor_generation") {
            if meta.generation.is_none() {
                meta.generation = Some(v);
            } else {
                meta.extra.insert("sensor_generation".to_owned(), v);
            }
        }

        meta.extra.extend(headers);

        Ok(meta)
    }
}

impl Deref for Headers {
    type Target = BTreeMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn parse_format_field(value: &str) -> Result<(EventEncoding, Geometry), MetadataError> {
    let mut parts = value.split(';');

    let format_name = parts
        .next()
        .ok_or_else(|| MetadataError::MalformedFormat(value.to_owned()))?;

    let encoding = match format_name {
        "EVT2" => EventEncoding::Evt20,
        "EVT21" => EventEncoding::Evt21,
        "EVT3" => EventEncoding::Evt30,
        other => return Err(MetadataError::UnknownFormat(other.to_owned())),
    };

    let mut width: Option<u16> = None;
    let mut height: Option<u16> = None;

    for part in parts {
        if let Some(val) = part.strip_prefix("height=") {
            height = Some(
                val.parse::<u16>()
                    .map_err(|_| MetadataError::InvalidNumber {
                        field: "height".to_owned(),
                        value: val.to_owned(),
                    })?,
            );
        } else if let Some(val) = part.strip_prefix("width=") {
            width = Some(
                val.parse::<u16>()
                    .map_err(|_| MetadataError::InvalidNumber {
                        field: "width".to_owned(),
                        value: val.to_owned(),
                    })?,
            );
        }
    }

    let geometry = Geometry {
        width: width.ok_or_else(|| MetadataError::MalformedFormat(value.to_owned()))?,
        height: height.ok_or_else(|| MetadataError::MalformedFormat(value.to_owned()))?,
    };

    Ok((encoding, geometry))
}

fn parse_geometry_field(value: &str) -> Result<Geometry, MetadataError> {
    let (w, h) = value
        .split_once('x')
        .ok_or_else(|| MetadataError::MalformedGeometry(value.to_owned()))?;

    let width = w.parse::<u16>().map_err(|_| MetadataError::InvalidNumber {
        field: "geometry.width".to_owned(),
        value: w.to_owned(),
    })?;
    let height = h.parse::<u16>().map_err(|_| MetadataError::InvalidNumber {
        field: "geometry.height".to_owned(),
        value: h.to_owned(),
    })?;

    Ok(Geometry { width, height })
}

fn encoding_from_evt_field(value: &str) -> Result<EventEncoding, MetadataError> {
    match value {
        "2.0" => Ok(EventEncoding::Evt20),
        "2.1" => Ok(EventEncoding::Evt21),
        "3.0" => Ok(EventEncoding::Evt30),
        other => Err(MetadataError::UnknownFormat(other.to_owned())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeaderEntry<'a> {
    KeyValue { key: &'a [u8], value: &'a [u8] },
    End,
}

fn header_line<'i>(input: &mut &'i [u8]) -> ModalResult<HeaderEntry<'i>> {
    let _ = b"% ".parse_next(input)?;
    let line = terminated(take_till(0.., b'\n'), b'\n').parse_next(input)?;

    let (key, value) = match line.iter().position(|&b| b == b' ') {
        Some(pos) => (&line[..pos], &line[pos + 1..]),
        None => (line, b"".as_ref()),
    };

    if key == b"end" {
        Ok(HeaderEntry::End)
    } else {
        Ok(HeaderEntry::KeyValue { key, value })
    }
}

/// # Examples
///
/// ```
/// use winnow::Parser;
/// use ebkit_raw::header::parse_header;
///
/// let input: &[u8] = b"% format EVT3;height=720;width=1280\n% date 2023-03-29 16:37:46\n% end\n";
/// let (remaining, headers) = parse_header.parse_peek(input).unwrap();
/// assert_eq!(headers["format"], "EVT3;height=720;width=1280");
/// assert_eq!(headers["date"], "2023-03-29 16:37:46");
/// assert!(remaining.is_empty());
/// ```
pub fn parse_header<'i>(input: &mut &'i [u8]) -> ModalResult<Headers> {
    let entries: Vec<HeaderEntry<'i>> = repeat(0.., header_line).parse_next(input)?;

    let mut map = BTreeMap::new();
    for entry in entries {
        match entry {
            HeaderEntry::KeyValue { key, value } => {
                let key = str::from_utf8(key)
                    .map_err(|_| winnow::error::ErrMode::Cut(winnow::error::ContextError::new()))?;
                let value = str::from_utf8(value)
                    .map_err(|_| winnow::error::ErrMode::Cut(winnow::error::ContextError::new()))?;
                map.insert(key.to_owned(), value.trim_end().to_owned());
            }
            HeaderEntry::End => break,
        }
    }

    Ok(Headers(map))
}

#[cfg(test)]
mod tests {
    use winnow::Parser;

    use super::*;

    #[test]
    fn parse_typical_header() {
        let input: &[u8] = b"% camera_integrator_name Prophesee\n\
            % date 2023-03-29 16:37:46\n\
            % evt 3.0\n\
            % format EVT3;height=720;width=1280\n\
            % generation 4.2\n\
            % geometry 1280x720\n\
            % integrator_name Prophesee\n\
            % plugin_integrator_name Prophesee\n\
            % plugin_name hal_plugin_imx636_evk4\n\
            % sensor_generation 4.2\n\
            % serial_number 00ca0009\n\
            % system_ID 49\n\
            % end\n";

        let result = parse_header.parse(input).unwrap();

        assert_eq!(result["camera_integrator_name"], "Prophesee");
        assert_eq!(result["date"], "2023-03-29 16:37:46");
        assert_eq!(result["evt"], "3.0");
        assert_eq!(result["format"], "EVT3;height=720;width=1280");
        assert_eq!(result["generation"], "4.2");
        assert_eq!(result["geometry"], "1280x720");
        assert_eq!(result["integrator_name"], "Prophesee");
        assert_eq!(result["plugin_integrator_name"], "Prophesee");
        assert_eq!(result["plugin_name"], "hal_plugin_imx636_evk4");
        assert_eq!(result["sensor_generation"], "4.2");
        assert_eq!(result["serial_number"], "00ca0009");
        assert_eq!(result["system_ID"], "49");
        assert_eq!(result.len(), 12);
    }

    #[test]
    fn stops_at_non_header_bytes() {
        let input: &[u8] = b"% format EVT3;height=720;width=1280\n% evt 3.0\n\x00\x01\x02";

        let (remaining, result) = parse_header.parse_peek(input).unwrap();

        assert_eq!(result["format"], "EVT3;height=720;width=1280");
        assert_eq!(result["evt"], "3.0");
        assert_eq!(remaining, b"\x00\x01\x02");
    }

    #[test]
    fn stops_at_end_marker() {
        let input: &[u8] = b"% format EVT2;height=480;width=640\n% end\n\x00\x01";

        let (remaining, result) = parse_header.parse_peek(input).unwrap();

        assert_eq!(result["format"], "EVT2;height=480;width=640");
        assert!(!result.contains_key("end"));
        assert_eq!(remaining, b"\x00\x01");
    }

    #[test]
    fn empty_header_before_binary() {
        let input: &[u8] = b"\x00\x01\x02\x03";

        let (remaining, result) = parse_header.parse_peek(input).unwrap();

        assert!(result.is_empty());
        assert_eq!(remaining, b"\x00\x01\x02\x03");
    }

    #[test]
    fn value_with_spaces() {
        let input: &[u8] = b"% date 2023-03-29 16:37:46\n% end\n";

        let result = parse_header.parse(input).unwrap();

        assert_eq!(result["date"], "2023-03-29 16:37:46");
    }

    #[test]
    fn key_without_value() {
        let input: &[u8] = b"% somekey\n% end\n";

        let result = parse_header.parse(input).unwrap();

        assert_eq!(result["somekey"], "");
    }

    #[test]
    fn end_only() {
        let input: &[u8] = b"% end\n";

        let result = parse_header.parse(input).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn duplicate_key_uses_last() {
        let input: &[u8] = b"% key first\n% key second\n% end\n";

        let result = parse_header.parse(input).unwrap();

        assert_eq!(result["key"], "second");
    }

    #[test]
    fn no_end_marker_at_eof() {
        let input: &[u8] = b"% format EVT3;height=720;width=1280\n% evt 3.0\n";

        let result = parse_header.parse(input).unwrap();

        assert_eq!(result["format"], "EVT3;height=720;width=1280");
        assert_eq!(result["evt"], "3.0");
    }

    #[test]
    fn into_metadata_full_header() {
        let input: &[u8] = b"% camera_integrator_name Prophesee\n\
            % date 2023-03-29 16:37:46\n\
            % evt 3.0\n\
            % format EVT3;height=720;width=1280\n\
            % generation 4.2\n\
            % geometry 1280x720\n\
            % integrator_name Prophesee\n\
            % plugin_integrator_name Prophesee\n\
            % plugin_name hal_plugin_imx636_evk4\n\
            % sensor_generation 4.2\n\
            % serial_number 00ca0009\n\
            % system_ID 49\n\
            % end\n";

        let headers = parse_header.parse(input).unwrap();
        let meta = headers.into_metadata().unwrap();

        assert_eq!(meta.encoding, Some(EventEncoding::Evt30));
        assert_eq!(
            meta.geometry,
            Geometry {
                width: 1280,
                height: 720
            }
        );
        assert_eq!(meta.camera_integrator_name.as_deref(), Some("Prophesee"));
        assert_eq!(meta.plugin_integrator_name.as_deref(), Some("Prophesee"));
        assert_eq!(meta.plugin_name.as_deref(), Some("hal_plugin_imx636_evk4"));
        assert_eq!(meta.serial_number.as_deref(), Some("00ca0009"));
        assert_eq!(meta.system_id.as_deref(), Some("49"));
        assert_eq!(meta.date.as_deref(), Some("2023-03-29 16:37:46"));
        assert_eq!(meta.generation.as_deref(), Some("4.2"));
    }

    #[test]
    fn into_metadata_evt_fallback() {
        let input: &[u8] = b"% evt 2.1\n% geometry 1280x720\n% end\n";

        let headers = parse_header.parse(input).unwrap();
        let meta = headers.into_metadata().unwrap();

        assert_eq!(meta.encoding, Some(EventEncoding::Evt21));
        assert_eq!(
            meta.geometry,
            Geometry {
                width: 1280,
                height: 720
            }
        );
    }

    #[test]
    fn into_metadata_missing_encoding() {
        let input: &[u8] = b"% geometry 1280x720\n% end\n";

        let headers = parse_header.parse(input).unwrap();
        let err = headers.into_metadata().unwrap_err();

        assert!(matches!(err, MetadataError::MissingEncoding));
    }

    #[test]
    fn into_metadata_unknown_keys_in_extra() {
        let input: &[u8] = b"% format EVT2;height=480;width=640\n% custom_key custom_val\n% end\n";

        let headers = parse_header.parse(input).unwrap();
        let meta = headers.into_metadata().unwrap();

        assert_eq!(meta.extra["custom_key"], "custom_val");
    }
}
