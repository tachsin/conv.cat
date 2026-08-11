//! The generic "unit ⇄ SI base" math shared by every in-scope category whose conversion is
//! expressible as a plain per-unit factor, an affine transform, or a reciprocal transform —
//! i.e. every category except `life_age` and `clothing_size`, which have their own modules.

use crate::{ConvertError, Format};

/// How one unit relates to its category's SI base unit. See [`super`] module docs for the
/// derivation of each variant's constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitConversion {
    /// `to_si(x) = x * factor_to_si`; `from_si(si) = si / factor_to_si`. The overwhelming
    /// majority of units in every category.
    Linear {
        /// Multiply by this to convert a value in this unit to the category's SI base.
        factor_to_si: f64,
    },
    /// `to_si(x) = x * scale + offset`; `from_si(si) = (si - offset) / scale`. Only
    /// `temperature` uses this today.
    Affine {
        /// Multiplicative coefficient.
        scale: f64,
        /// Additive coefficient, applied after scaling.
        offset: f64,
    },
    /// `to_si(x) = k / x`; `from_si(si) = k / si` — a self-inverse (involution) transform, e.g.
    /// km-per-liter-style fuel economy units. Only `fuel_consumption` uses this today.
    Reciprocal {
        /// The constant `k` in `k / x`.
        k: f64,
    },
    /// This unit has no conversion data in the legacy catalog either — see
    /// `packages/data/src/units/README.md` for the full list and why. Every conversion naming
    /// this unit returns [`ConvertError::UnsupportedFeature`], never a fabricated result.
    Unconvertible,
}

/// One unit's catalog entry within a [`CategoryTable`].
pub struct UnitEntry {
    /// Stable lowercase id, matching `packages/data`'s catalog (e.g. `"celsius"`, `"meter"`).
    pub id: &'static str,
    /// How this unit relates to the category's SI base — see [`UnitConversion`].
    pub conversion: UnitConversion,
}

/// One category's full unit table, as looked up by [`super::converter::UnitsConverter`].
pub struct CategoryTable {
    /// The category id, e.g. `"length"` — matches `packages/data`'s catalog id and the
    /// `units_<category_id>` suffix of the corresponding [`Format::id`].
    pub category_id: &'static str,
    /// Every unit in this category.
    pub units: &'static [UnitEntry],
}

impl CategoryTable {
    /// Looks up a unit by id within this category.
    pub fn find(&self, unit_id: &str) -> Option<&'static UnitEntry> {
        self.units.iter().find(|entry| entry.id == unit_id)
    }
}

fn to_si(conversion: UnitConversion, value: f64) -> Option<f64> {
    let result = match conversion {
        UnitConversion::Linear { factor_to_si } => {
            if !factor_to_si.is_finite() {
                return None;
            }
            value * factor_to_si
        }
        UnitConversion::Affine { scale, offset } => {
            if !scale.is_finite() || !offset.is_finite() {
                return None;
            }
            value * scale + offset
        }
        UnitConversion::Reciprocal { k } => {
            if !k.is_finite() || value == 0.0 {
                return None;
            }
            k / value
        }
        UnitConversion::Unconvertible => return None,
    };
    result.is_finite().then_some(result)
}

fn from_si(conversion: UnitConversion, si: f64) -> Option<f64> {
    let result = match conversion {
        UnitConversion::Linear { factor_to_si } => {
            if !factor_to_si.is_finite() || factor_to_si == 0.0 {
                return None;
            }
            si / factor_to_si
        }
        UnitConversion::Affine { scale, offset } => {
            if !scale.is_finite() || !offset.is_finite() || scale == 0.0 {
                return None;
            }
            (si - offset) / scale
        }
        UnitConversion::Reciprocal { k } => {
            if !k.is_finite() || si == 0.0 {
                return None;
            }
            k / si
        }
        UnitConversion::Unconvertible => return None,
    };
    result.is_finite().then_some(result)
}

/// Converts `value` from `from_unit` to `to_unit` within `table`, via the category's SI base.
///
/// `from_unit == to_unit` short-circuits to `Ok(value)` unconditionally — even for an unknown or
/// [`UnitConversion::Unconvertible`] unit id — matching the legacy Rust/TS behavior this crate
/// ports (`conv.cat_legacy/conv-rust/.../units/convert.rs`'s `convert_units`, which checks
/// `from_id == to_id` before ever looking a unit up). A conversion to itself is always a
/// meaningful no-op regardless of whether real conversion data exists for that unit.
///
/// Otherwise: an unrecognized unit id or one with [`UnitConversion::Unconvertible`] returns
/// [`ConvertError::UnsupportedFeature`]; a non-finite input, or a value that produces a
/// non-finite result at either hop (e.g. `0` on a [`UnitConversion::Reciprocal`] unit), returns
/// [`ConvertError::MalformedInput`].
pub fn convert_via_si(
    table: &CategoryTable,
    from_unit: &str,
    to_unit: &str,
    value: f64,
    format: Format,
) -> Result<f64, ConvertError> {
    if from_unit == to_unit {
        return Ok(value);
    }
    if !value.is_finite() {
        return Err(ConvertError::MalformedInput { format });
    }

    let from = table
        .find(from_unit)
        .ok_or(ConvertError::UnsupportedFeature {
            format,
            feature: "unknown_unit",
        })?;
    let to = table
        .find(to_unit)
        .ok_or(ConvertError::UnsupportedFeature {
            format,
            feature: "unknown_unit",
        })?;

    if matches!(from.conversion, UnitConversion::Unconvertible)
        || matches!(to.conversion, UnitConversion::Unconvertible)
    {
        return Err(ConvertError::UnsupportedFeature {
            format,
            feature: "no_conversion_data",
        });
    }

    let si = to_si(from.conversion, value).ok_or(ConvertError::MalformedInput { format })?;
    from_si(to.conversion, si).ok_or(ConvertError::MalformedInput { format })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTH: CategoryTable = CategoryTable {
        category_id: "length",
        units: &[
            UnitEntry {
                id: "meter",
                conversion: UnitConversion::Linear { factor_to_si: 1.0 },
            },
            UnitEntry {
                id: "foot",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.3048,
                },
            },
            UnitEntry {
                id: "mystery",
                conversion: UnitConversion::Unconvertible,
            },
        ],
    };

    #[test]
    fn linear_round_trip_is_close_to_identity() {
        let converted = convert_via_si(&LENGTH, "meter", "foot", 1.0, Format::PlainText).unwrap();
        assert!((converted - 3.280_839_895).abs() < 1e-9);
        let back = convert_via_si(&LENGTH, "foot", "meter", converted, Format::PlainText).unwrap();
        assert!((back - 1.0).abs() < 1e-9);
    }

    #[test]
    fn identity_short_circuits_even_for_unconvertible_units() {
        let result =
            convert_via_si(&LENGTH, "mystery", "mystery", 42.0, Format::PlainText).unwrap();
        assert_eq!(result, 42.0);
    }

    #[test]
    fn unconvertible_unit_is_unsupported_feature_across_a_real_pair() {
        let err = convert_via_si(&LENGTH, "mystery", "meter", 1.0, Format::PlainText).unwrap_err();
        assert!(matches!(
            err,
            ConvertError::UnsupportedFeature {
                feature: "no_conversion_data",
                ..
            }
        ));
    }

    #[test]
    fn unknown_unit_is_unsupported_feature() {
        let err = convert_via_si(&LENGTH, "nope", "meter", 1.0, Format::PlainText).unwrap_err();
        assert!(matches!(
            err,
            ConvertError::UnsupportedFeature {
                feature: "unknown_unit",
                ..
            }
        ));
    }
}
