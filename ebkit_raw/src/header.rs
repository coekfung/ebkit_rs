//! RAW file header parser — see `spec/raw.md`.

use ebkit_core::{EventEncoding, Geometry, Metadata, MetadataBuilder};
use winnow::ascii::dec_uint;
use winnow::combinator::{alt, cut_err, delimited, fail, separated_pair, terminated};
use winnow::error::StrContext;
use winnow::token::{literal, take_till};
use winnow::{ModalResult, Parser};

pub fn raw_headers<'i>(input: &mut &'i [u8]) -> ModalResult<Metadata> {
    let mut builder = MetadataBuilder::new();

    loop {
        if !literal::<_, _, ()>(b"% ").parse_next(input).is_ok() {
            break;
        }
        if literal::<_, _, ()>(b"end\n").parse_next(input).is_ok() {
            break;
        }

        let key: String = terminated(take_till(1.., b' '), b' ')
            .parse_to()
            .parse_next(input)?;

        let mut str_value = terminated(take_till(1.., b'\n'), b'\n').parse_to();

        builder = match key.as_ref() {
            "format" => {
                let (format, height, width) = (
                    terminated(
                        alt((
                            b"EVT2".value(EventEncoding::Evt20),
                            b"EVT21".value(EventEncoding::Evt21),
                            b"EVT3".value(EventEncoding::Evt30),
                        )),
                        b';',
                    ),
                    delimited(b"height=", dec_uint, b';'),
                    delimited(b"width=", dec_uint, b'\n'),
                )
                    .parse_next(input)?;
                builder
                    .with_encoding(format)
                    .with_geometry(Geometry { width, height })
            }
            "evt" => builder.with_encoding(
                terminated(
                    alt((
                        b"2.0".value(EventEncoding::Evt20),
                        b"2.1".value(EventEncoding::Evt21),
                        b"3.0".value(EventEncoding::Evt30),
                    )),
                    b'\n',
                )
                .parse_next(input)?,
            ),
            "geometry" => builder.with_geometry(
                terminated(separated_pair(dec_uint, b'x', dec_uint), b'\n')
                    .map(|(width, height)| Geometry { width, height })
                    .parse_next(input)?,
            ),
            "camera_integrator_name" => {
                builder.with_camera_integrator_name(str_value.parse_next(input)?)
            }
            "integrator_name" => {
                let name = str_value.parse_next(input)?;
                if builder.camera_integrator_name().is_none() {
                    builder.with_camera_integrator_name(name)
                } else {
                    builder
                }
            }
            "plugin_integrator_name" => {
                builder.with_plugin_integrator_name(str_value.parse_next(input)?)
            }
            "plugin_name" => builder.with_plugin_name(str_value.parse_next(input)?),
            "serial_number" => builder.with_serial_number(str_value.parse_next(input)?),
            "system_ID" => builder.with_system_id(str_value.parse_next(input)?),
            "date" => builder.with_date(str_value.parse_next(input)?),
            "generation" => builder.with_generation(str_value.parse_next(input)?),
            "sensor_generation" => {
                let genernation = str_value.parse_next(input)?;
                if builder.generation().is_none() {
                    builder.with_generation(genernation)
                } else {
                    builder
                }
            }
            _ => builder.with_extra(key.to_owned(), str_value.parse_next(input)?),
        };
    }

    if builder.encoding().is_none() {
        return cut_err(fail)
            .context(StrContext::Label("missing encoding in raw header"))
            .parse_next(input);
    }

    if builder.geometry().is_none() {
        return cut_err(fail)
            .context(StrContext::Label("missing geometry in raw header"))
            .parse_next(input);
    }

    match builder.build() {
        Some(metadata) => Ok(metadata),
        None => cut_err(fail)
            .context(StrContext::Label("failed to build metadata"))
            .parse_next(input),
    }
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

        let result = raw_headers.parse(input).unwrap();

        assert_eq!(result.encoding, Some(EventEncoding::Evt30));
        assert_eq!(result.geometry.width, 1280);
        assert_eq!(result.geometry.height, 720);
        assert_eq!(result.camera_integrator_name.as_deref(), Some("Prophesee"));
        assert_eq!(result.plugin_integrator_name.as_deref(), Some("Prophesee"));
        assert_eq!(
            result.plugin_name.as_deref(),
            Some("hal_plugin_imx636_evk4")
        );
        assert_eq!(result.serial_number.as_deref(), Some("00ca0009"));
        assert_eq!(result.system_id.as_deref(), Some("49"));
        assert_eq!(result.date.as_deref(), Some("2023-03-29 16:37:46"));
        assert_eq!(result.generation.as_deref(), Some("4.2"));
    }

    #[test]
    fn stops_at_non_header_bytes() {
        let input: &[u8] = b"% format EVT3;height=720;width=1280\n% evt 3.0\n\x00\x01\x02";

        let (remaining, result) = raw_headers.parse_peek(input).unwrap();

        assert_eq!(result.encoding, Some(EventEncoding::Evt30));
        assert_eq!(result.geometry.width, 1280);
        assert_eq!(result.geometry.height, 720);
        assert_eq!(remaining, b"\x00\x01\x02");
    }

    #[test]
    fn stops_at_end_marker() {
        let input: &[u8] = b"% format EVT2;height=480;width=640\n% end\n\x00\x01";

        let (remaining, result) = raw_headers.parse_peek(input).unwrap();

        assert_eq!(result.encoding, Some(EventEncoding::Evt20));
        assert_eq!(result.geometry.width, 640);
        assert_eq!(result.geometry.height, 480);
        assert_eq!(remaining, b"\x00\x01");
    }

    #[test]
    fn missing_encoding_fails() {
        let input: &[u8] = b"% geometry 1280x720\n% end\n";

        assert!(raw_headers.parse(input).is_err());
    }

    #[test]
    fn evt_fallback_works() {
        let input: &[u8] = b"% evt 2.1\n% geometry 1280x720\n% end\n";

        let result = raw_headers.parse(input).unwrap();

        assert_eq!(result.encoding, Some(EventEncoding::Evt21));
        assert_eq!(result.geometry.width, 1280);
        assert_eq!(result.geometry.height, 720);
    }

    #[test]
    fn unknown_keys_land_in_extra() {
        let input: &[u8] = b"% format EVT2;height=480;width=640\n% custom_key custom_val\n% end\n";

        let result = raw_headers.parse(input).unwrap();

        assert_eq!(result.extra["custom_key"], "custom_val");
    }
}
