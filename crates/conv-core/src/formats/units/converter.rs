//! [`UnitsConverter`]: the single, stateless [`Converter`] registered for every in-scope unit
//! `Format` self-pair. Dispatches on `from` (always equal to `to` for a self-pair — see the
//! parent module's docs) to the right conversion model.

use crate::{ConvertError, ConvertOptions, Converter, Format};

use super::catalog::convert_via_si;
use super::payload::{format_number, parse_number, parse_request};
use super::{clothing_size, generated, life_age};

/// Stateless — registered for every in-scope unit `Format` self-pair in
/// `crate::default_registry`. Single-pass and fast (a table lookup plus a handful of float ops,
/// or a small chart scan for `clothing_size`), so — like [`crate::formats::identity::IdentityConverter`]
/// — it checks cancellation once up front rather than polling mid-conversion.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnitsConverter;

impl Converter for UnitsConverter {
    fn convert(
        &self,
        input: &[u8],
        from: Format,
        to: Format,
        options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }

        let category_id =
            category_id_for(from).ok_or(ConvertError::UnsupportedPair { from, to })?;
        let request = parse_request(input, from)?;

        let output_text = if category_id == "life_age" {
            let value = parse_number(request.raw_value, from)?;
            let result = life_age::convert(request.from_unit, request.to_unit, value, from)?;
            format_number(result)
        } else if category_id == "clothing_size" {
            let result = clothing_size::convert_from_input(
                request.raw_value,
                request.from_unit,
                request.to_unit,
                from,
            )?;
            match result {
                clothing_size::ClothingValue::Number(n) => format_number(n),
                clothing_size::ClothingValue::Label(l) => l,
            }
        } else {
            // One of the four generic-model categories — `generated::CATEGORIES` always has an
            // entry for `category_id` here, since `category_id_for` and the codegen script are
            // both driven off the same eight-category list; a missing entry would mean the two
            // have drifted, not that the caller did anything wrong, so this is `Internal`, not
            // `UnsupportedPair`.
            let table = generated::CATEGORIES
                .iter()
                .find(|t| t.category_id == category_id)
                .ok_or(ConvertError::Internal {
                    detail: "units category missing from generated catalog",
                })?;
            let value = parse_number(request.raw_value, from)?;
            let result = convert_via_si(table, request.from_unit, request.to_unit, value, from)?;
            format_number(result)
        };

        options.report_progress(1.0);
        Ok(output_text.into_bytes())
    }
}

/// Maps a `Format::Units*` variant to its category id — the join key with both
/// [`generated::CATEGORIES`] and `packages/data`'s catalog. `None` for any other `Format`
/// (unreachable through the default registry, which only ever calls this converter with a
/// `Format::Units*` `from` — guarded here anyway since `UnitsConverter` is a public type a caller
/// could invoke directly with an arbitrary `Format`).
fn category_id_for(format: Format) -> Option<&'static str> {
    match format {
        Format::UnitsLength => Some("length"),
        Format::UnitsMass => Some("mass"),
        Format::UnitsVolume => Some("volume"),
        Format::UnitsCooking => Some("cooking"),
        Format::UnitsTemperature => Some("temperature"),
        Format::UnitsFuelConsumption => Some("fuel_consumption"),
        Format::UnitsLifeAge => Some("life_age"),
        Format::UnitsClothingSize => Some("clothing_size"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConvertOptions;

    #[test]
    fn converts_length_end_to_end() {
        let output = UnitsConverter
            .convert(
                b"meter|foot|1",
                Format::UnitsLength,
                Format::UnitsLength,
                &ConvertOptions::default(),
            )
            .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "3.280839895013123");
    }

    #[test]
    fn converts_temperature_offset_end_to_end() {
        // fahrenheit -> celsius is bit-exact for this value (verified) — see
        // `super::super::mod`'s "Precision" docs and `celsius_to_fahrenheit_has_tiny_but_bounded_fp_error`
        // below for the *other* direction, which is not, and why that's expected rather than a bug.
        let output = UnitsConverter
            .convert(
                b"fahrenheit|celsius|32",
                Format::UnitsTemperature,
                Format::UnitsTemperature,
                &ConvertOptions::default(),
            )
            .unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "0");
    }

    #[test]
    fn celsius_to_fahrenheit_has_tiny_but_bounded_fp_error() {
        // Celsius (scale 1, offset 273.15 — both exact decimal literals) and Fahrenheit (scale
        // 5/9, offset 459.67*5/9 — neither exactly representable in binary floating point) don't
        // cancel bit-for-bit when composed through the shared Kelvin intermediate: verified
        // in-session that 0°C -> °F yields 31.999999999999986, not exactly 32. This is expected
        // IEEE 754 behavior for a two-hop affine composition through decimal literals, not a
        // converter bug — the fahrenheit -> celsius direction for the same values happens to be
        // bit-exact (see the test above), which is itself evidence this is rounding noise, not a
        // wrong formula. This is the concrete case `formats::units` module docs' "Precision"
        // section points at.
        let output = UnitsConverter
            .convert(
                b"celsius|fahrenheit|0",
                Format::UnitsTemperature,
                Format::UnitsTemperature,
                &ConvertOptions::default(),
            )
            .unwrap();
        let value: f64 = String::from_utf8(output).unwrap().parse().unwrap();
        assert!(
            (value - 32.0).abs() < 1e-9,
            "expected ~32 within 1e-9, got {value}"
        );
    }

    #[test]
    fn rejects_a_format_this_converter_does_not_know() {
        let err = UnitsConverter
            .convert(
                b"a|b|1",
                Format::PlainText,
                Format::PlainText,
                &ConvertOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(err, ConvertError::UnsupportedPair { .. }));
    }
}
