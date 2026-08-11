//! The wire protocol every unit [`crate::Converter`] speaks: plain UTF-8 text, not JSON —
//! `conv-core` has zero runtime dependencies (see the crate-level docs), and this protocol is
//! simple enough not to need one.
//!
//! **Request** (input bytes): `"<from_unit_id>|<to_unit_id>|<value>"`, e.g.
//! `"celsius|fahrenheit|0"`.
//!
//! **Response** (output bytes): the result as text, e.g. `"32"`.
//!
//! `<value>`/the response is a decimal number for every in-scope category except
//! `clothing_size`, which additionally accepts/produces a size-label token (`"M"`, `"42"`) — see
//! [`super::clothing_size`], which parses `raw_value` itself rather than going through
//! [`parse_number`].

use crate::{ConvertError, Format};

/// A parsed request: the two unit ids plus the raw (unparsed) value/label text.
pub struct Request<'a> {
    /// The unit id `value` is currently expressed in.
    pub from_unit: &'a str,
    /// The unit id to convert to.
    pub to_unit: &'a str,
    /// The value or label field, exactly as sent — not yet parsed as a number, since
    /// `clothing_size` needs to see it as text first.
    pub raw_value: &'a str,
}

/// Splits `input` into a [`Request`]. Returns [`ConvertError::MalformedInput`] if it isn't valid
/// UTF-8, doesn't have exactly three `|`-delimited fields, or any field is empty.
pub fn parse_request(input: &[u8], format: Format) -> Result<Request<'_>, ConvertError> {
    let text = std::str::from_utf8(input).map_err(|_| ConvertError::MalformedInput { format })?;

    let mut parts = text.split('|');
    let (Some(from_unit), Some(to_unit), Some(raw_value), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ConvertError::MalformedInput { format });
    };

    if from_unit.is_empty() || to_unit.is_empty() || raw_value.is_empty() {
        return Err(ConvertError::MalformedInput { format });
    }

    Ok(Request {
        from_unit,
        to_unit,
        raw_value,
    })
}

/// Parses `raw_value` as a finite decimal number. Maps unparseable text, `NaN`, or infinity to
/// [`ConvertError::MalformedInput`] — every category except `clothing_size` requires a number.
pub fn parse_number(raw_value: &str, format: Format) -> Result<f64, ConvertError> {
    let value: f64 = raw_value
        .parse()
        .map_err(|_| ConvertError::MalformedInput { format })?;
    if !value.is_finite() {
        return Err(ConvertError::MalformedInput { format });
    }
    Ok(value)
}

/// Encodes a numeric result as the canonical wire text: Rust's `f64::to_string()`, the shortest
/// decimal that round-trips back to the same `f64` — verified (see the units vertical-slice
/// ticket notes): no trailing `.0` on whole numbers (`32.0.to_string() == "32"`), full precision
/// otherwise (`(1.0 / 3.0).to_string() == "0.3333333333333333"`). Deterministic, so golden
/// fixtures built from this stay byte-exact across runs and platforms.
pub fn format_number(value: f64) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_request() {
        let req = parse_request(b"celsius|fahrenheit|0", Format::PlainText).unwrap();
        assert_eq!(req.from_unit, "celsius");
        assert_eq!(req.to_unit, "fahrenheit");
        assert_eq!(req.raw_value, "0");
    }

    #[test]
    fn rejects_wrong_field_count() {
        assert!(parse_request(b"celsius|fahrenheit", Format::PlainText).is_err());
        assert!(parse_request(b"celsius|fahrenheit|0|extra", Format::PlainText).is_err());
        assert!(parse_request(b"", Format::PlainText).is_err());
    }

    #[test]
    fn rejects_empty_fields() {
        assert!(parse_request(b"|fahrenheit|0", Format::PlainText).is_err());
        assert!(parse_request(b"celsius||0", Format::PlainText).is_err());
        assert!(parse_request(b"celsius|fahrenheit|", Format::PlainText).is_err());
    }

    #[test]
    fn rejects_invalid_utf8() {
        assert!(parse_request(&[0xff, 0xfe, b'|', b'a', b'|', b'1'], Format::PlainText).is_err());
    }

    #[test]
    fn parse_number_rejects_non_numeric_and_non_finite() {
        assert!(parse_number("not-a-number", Format::PlainText).is_err());
        assert!(parse_number("NaN", Format::PlainText).is_err());
        assert!(parse_number("inf", Format::PlainText).is_err());
        assert_eq!(parse_number("42.5", Format::PlainText).unwrap(), 42.5);
    }

    #[test]
    fn format_number_matches_the_documented_round_trip_shape() {
        assert_eq!(format_number(32.0), "32");
        assert_eq!(format_number(273.15), "273.15");
        assert_eq!(format_number(-40.0), "-40");
    }
}
