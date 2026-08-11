//! GENERATED — do not hand-edit.
//!
//! Source: `packages/data/src/units/categories/{length,mass,volume,cooking,temperature,fuel_consumption}.json`.
//! Regenerate with `pnpm generate:units` (runs `scripts/generate-units-catalog.mjs`) after
//! editing any of those files, then review the diff before committing — see
//! `packages/data/src/units/README.md` for the full regeneration rule and the honest list of
//! units with no conversion data (marked [`UnitConversion::Unconvertible`] below, each with a
//! comment explaining why).
//!
//! Generated 2026-08-11T11:14:29.035Z.

use super::catalog::{CategoryTable, UnitConversion, UnitEntry};

/// Every "generic model" category's unit table — [`super::converter::UnitsConverter`] looks this
/// up by category id for every category except `life_age`/`clothing_size`, which are hand-ported
/// algorithms in their own modules instead of table data.
pub static CATEGORIES: &[CategoryTable] = &[
    CategoryTable {
        category_id: "length",
        units: &[
            UnitEntry {
                id: "angstrom",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-10,
                },
            },
            UnitEntry {
                id: "astronomical_unit",
                conversion: UnitConversion::Linear {
                    factor_to_si: 149600000000.0,
                },
            },
            UnitEntry {
                id: "barleycorn",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.008467,
                },
            },
            UnitEntry {
                id: "centimeter",
                conversion: UnitConversion::Linear { factor_to_si: 0.01 },
            },
            UnitEntry {
                id: "chain",
                conversion: UnitConversion::Linear {
                    factor_to_si: 20.1168,
                },
            },
            UnitEntry {
                id: "cubit",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.4572,
                },
            },
            UnitEntry {
                id: "cun_chinese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0333,
                },
            },
            UnitEntry {
                id: "fathom",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1.8288,
                },
            },
            UnitEntry {
                id: "femtometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-15,
                },
            },
            UnitEntry {
                id: "foot",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.3048,
                },
            },
            UnitEntry {
                id: "furlong",
                conversion: UnitConversion::Linear {
                    factor_to_si: 201.168,
                },
            },
            UnitEntry {
                id: "hand",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.1016,
                },
            },
            UnitEntry {
                id: "inch",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0254,
                },
            },
            UnitEntry {
                id: "kilometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000.0,
                },
            },
            UnitEntry {
                id: "kiloparsec",
                conversion: UnitConversion::Linear {
                    factor_to_si: 30860000000000000000.0,
                },
            },
            UnitEntry {
                id: "league",
                conversion: UnitConversion::Linear {
                    factor_to_si: 4828.032,
                },
            },
            UnitEntry {
                id: "li_chinese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 500.0,
                },
            },
            UnitEntry {
                id: "light_year",
                conversion: UnitConversion::Linear {
                    factor_to_si: 9461000000000000.0,
                },
            },
            UnitEntry {
                id: "megaparsec",
                conversion: UnitConversion::Linear {
                    factor_to_si: 3.086e+22,
                },
            },
            UnitEntry {
                id: "meter",
                conversion: UnitConversion::Linear { factor_to_si: 1.0 },
            },
            UnitEntry {
                id: "micrometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000001,
                },
            },
            UnitEntry {
                id: "mile",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1609.344,
                },
            },
            UnitEntry {
                id: "millimeter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.001,
                },
            },
            UnitEntry {
                id: "nanometer",
                conversion: UnitConversion::Linear { factor_to_si: 1e-9 },
            },
            UnitEntry {
                id: "nautical_mile",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1852.0,
                },
            },
            UnitEntry {
                id: "parsec",
                conversion: UnitConversion::Linear {
                    factor_to_si: 30860000000000000.0,
                },
            },
            UnitEntry {
                id: "pica",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.004233,
                },
            },
            UnitEntry {
                id: "picometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-12,
                },
            },
            UnitEntry {
                id: "plank_length",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1.616e-35,
                },
            },
            UnitEntry {
                id: "point_typography",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000352778,
                },
            },
            UnitEntry {
                id: "ri_japanese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 3927.27,
                },
            },
            UnitEntry {
                id: "rod",
                conversion: UnitConversion::Linear {
                    factor_to_si: 5.0292,
                },
            },
            UnitEntry {
                id: "shaku",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.303,
                },
            },
            UnitEntry {
                id: "span",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.2286,
                },
            },
            UnitEntry {
                id: "stadion",
                conversion: UnitConversion::Linear {
                    factor_to_si: 185.0,
                },
            },
            UnitEntry {
                id: "sun_japanese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0303,
                },
            },
            UnitEntry {
                id: "thou",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0000254,
                },
            },
            UnitEntry {
                id: "yard",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.9144,
                },
            },
            UnitEntry {
                id: "yoctometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-24,
                },
            },
            UnitEntry {
                id: "zeptometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-21,
                },
            },
            UnitEntry {
                id: "attometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-18,
                },
            },
            UnitEntry {
                id: "petameter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000000000000000.0,
                },
            },
            UnitEntry {
                id: "exameter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000000000000000000.0,
                },
            },
            UnitEntry {
                id: "zhang_chinese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 3.333,
                },
            },
            UnitEntry {
                id: "chi_chinese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.3333,
                },
            },
            UnitEntry {
                id: "ken_japan",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1.818,
                },
            },
            UnitEntry {
                id: "shaku_japan",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.303,
                },
            },
            UnitEntry {
                id: "vara",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.8359,
                },
            },
            UnitEntry {
                id: "nautical_mile_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1853.184,
                },
            },
            UnitEntry {
                id: "planck_length",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1.616255e-35,
                },
            },
        ],
    },
    CategoryTable {
        category_id: "mass",
        units: &[
            UnitEntry {
                id: "carat",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0002,
                },
            },
            UnitEntry {
                id: "dalton",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1.66054e-27,
                },
            },
            UnitEntry {
                id: "dram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00177185,
                },
            },
            UnitEntry {
                id: "electron_mass",
                conversion: UnitConversion::Linear {
                    factor_to_si: 9.109e-31,
                },
            },
            UnitEntry {
                id: "grain",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0000647989,
                },
            },
            UnitEntry {
                id: "gram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.001,
                },
            },
            UnitEntry {
                id: "jin_chinese",
                conversion: UnitConversion::Linear { factor_to_si: 0.5 },
            },
            UnitEntry {
                id: "kan_japanese",
                conversion: UnitConversion::Linear { factor_to_si: 3.75 },
            },
            UnitEntry {
                id: "kilogram",
                conversion: UnitConversion::Linear { factor_to_si: 1.0 },
            },
            UnitEntry {
                id: "long_ton",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1016.05,
                },
            },
            UnitEntry {
                id: "microgram",
                conversion: UnitConversion::Linear { factor_to_si: 1e-9 },
            },
            UnitEntry {
                id: "milligram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000001,
                },
            },
            UnitEntry {
                id: "mina_ancient",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.4333,
                },
            },
            UnitEntry {
                id: "ounce",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0283495,
                },
            },
            UnitEntry {
                id: "pennyweight",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00155517,
                },
            },
            UnitEntry {
                id: "planck_mass",
                conversion: UnitConversion::Linear {
                    factor_to_si: 2.176e-8,
                },
            },
            UnitEntry {
                id: "pound",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.453592,
                },
            },
            UnitEntry {
                id: "quintal",
                conversion: UnitConversion::Linear {
                    factor_to_si: 100.0,
                },
            },
            UnitEntry {
                id: "scruple",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00129958,
                },
            },
            UnitEntry {
                id: "shekel_ancient",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.01133,
                },
            },
            UnitEntry {
                id: "short_ton",
                conversion: UnitConversion::Linear {
                    factor_to_si: 907.185,
                },
            },
            UnitEntry {
                id: "slug",
                conversion: UnitConversion::Linear {
                    factor_to_si: 14.5939,
                },
            },
            UnitEntry {
                id: "solar_mass",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1.989e+30,
                },
            },
            UnitEntry {
                id: "stone",
                conversion: UnitConversion::Linear {
                    factor_to_si: 6.35029,
                },
            },
            UnitEntry {
                id: "talent_ancient",
                conversion: UnitConversion::Linear { factor_to_si: 26.0 },
            },
            UnitEntry {
                id: "tonne",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000.0,
                },
            },
            UnitEntry {
                id: "troy_ounce",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0311035,
                },
            },
            UnitEntry {
                id: "troy_pound",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.373242,
                },
            },
            UnitEntry {
                id: "yoctogram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-27,
                },
            },
            UnitEntry {
                id: "zeptogram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-24,
                },
            },
            UnitEntry {
                id: "attogram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-21,
                },
            },
            UnitEntry {
                id: "femtogram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1e-18,
                },
            },
            UnitEntry {
                id: "petagram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000000000000.0,
                },
            },
            UnitEntry {
                id: "exagram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000000000000000.0,
                },
            },
            UnitEntry {
                id: "momme",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00375,
                },
            },
            UnitEntry {
                id: "tael",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0377994,
                },
            },
            UnitEntry {
                id: "catty",
                conversion: UnitConversion::Linear { factor_to_si: 0.5 },
            },
            UnitEntry {
                id: "stone_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 6.35029,
                },
            },
            UnitEntry {
                id: "quarter_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 12.7006,
                },
            },
            UnitEntry {
                id: "hundredweight_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 50.8023,
                },
            },
            UnitEntry {
                id: "hundredweight_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 45.3592,
                },
            },
        ],
    },
    CategoryTable {
        category_id: "volume",
        units: &[
            UnitEntry {
                id: "barrel_oil",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.158987,
                },
            },
            UnitEntry {
                id: "barrel_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.11924,
                },
            },
            UnitEntry {
                id: "bushel_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.035239,
                },
            },
            UnitEntry {
                id: "centiliter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00001,
                },
            },
            UnitEntry {
                id: "cord",
                conversion: UnitConversion::Linear {
                    factor_to_si: 3.62456,
                },
            },
            UnitEntry {
                id: "cubic_centimeter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000001,
                },
            },
            UnitEntry {
                id: "cubic_foot",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0283168,
                },
            },
            UnitEntry {
                id: "cubic_inch",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0000163871,
                },
            },
            UnitEntry {
                id: "cubic_kilometer",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000000000.0,
                },
            },
            UnitEntry {
                id: "cubic_meter",
                conversion: UnitConversion::Linear { factor_to_si: 1.0 },
            },
            UnitEntry {
                id: "cubic_mile",
                conversion: UnitConversion::Linear {
                    factor_to_si: 4168180000.0,
                },
            },
            UnitEntry {
                id: "cubic_yard",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.764555,
                },
            },
            UnitEntry {
                id: "cup_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000236588,
                },
            },
            UnitEntry {
                id: "deciliter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0001,
                },
            },
            UnitEntry {
                id: "drop",
                conversion: UnitConversion::Linear { factor_to_si: 5e-8 },
            },
            UnitEntry {
                id: "firkin",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.034069,
                },
            },
            UnitEntry {
                id: "fluid_ounce_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0000284131,
                },
            },
            UnitEntry {
                id: "fluid_ounce_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0000295735,
                },
            },
            UnitEntry {
                id: "gallon_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00454609,
                },
            },
            UnitEntry {
                id: "gallon_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00378541,
                },
            },
            UnitEntry {
                id: "gill_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000142065,
                },
            },
            UnitEntry {
                id: "gill_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000118294,
                },
            },
            UnitEntry {
                id: "hogshead",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.238481,
                },
            },
            UnitEntry {
                id: "lambda",
                conversion: UnitConversion::Linear { factor_to_si: 1e-9 },
            },
            UnitEntry {
                id: "liter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.001,
                },
            },
            UnitEntry {
                id: "milliliter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000001,
                },
            },
            UnitEntry {
                id: "peck",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00880977,
                },
            },
            UnitEntry {
                id: "pint_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000568261,
                },
            },
            UnitEntry {
                id: "pint_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000473176,
                },
            },
            UnitEntry {
                id: "quart_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.000946353,
                },
            },
            UnitEntry {
                id: "sheng_chinese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.001,
                },
            },
            UnitEntry {
                id: "sho_japanese",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.001804,
                },
            },
            UnitEntry {
                id: "tablespoon_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.0000147868,
                },
            },
            UnitEntry {
                id: "teaspoon_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00000492892,
                },
            },
            UnitEntry {
                id: "cubic_millimeter",
                conversion: UnitConversion::Linear { factor_to_si: 1e-9 },
            },
            UnitEntry {
                id: "cubic_decimeter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.001,
                },
            },
            UnitEntry {
                id: "fluid_dram",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00369669,
                },
            },
            UnitEntry {
                id: "minim",
                conversion: UnitConversion::Linear {
                    factor_to_si: 6.161152e-8,
                },
            },
            UnitEntry {
                id: "board_foot",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.00235974,
                },
            },
            UnitEntry {
                id: "register_ton",
                conversion: UnitConversion::Linear {
                    factor_to_si: 2.83168,
                },
            },
            UnitEntry {
                id: "acre_foot",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1233.48,
                },
            },
            UnitEntry {
                id: "beer_barrel_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.1173478,
                },
            },
            UnitEntry {
                id: "oil_barrel",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.1589873,
                },
            },
        ],
    },
    CategoryTable {
        category_id: "cooking",
        units: &[
            UnitEntry {
                id: "cup_metric",
                conversion: UnitConversion::Linear {
                    factor_to_si: 250.0,
                },
            },
            UnitEntry {
                id: "cup_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 284.131,
                },
            },
            UnitEntry {
                id: "cup_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 236.588,
                },
            },
            UnitEntry {
                id: "dash",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.616,
                },
            },
            UnitEntry {
                id: "dessertspoon",
                conversion: UnitConversion::Linear { factor_to_si: 10.0 },
            },
            UnitEntry {
                id: "jigger",
                conversion: UnitConversion::Linear {
                    factor_to_si: 44.36,
                },
            },
            UnitEntry {
                id: "milliliter",
                conversion: UnitConversion::Linear { factor_to_si: 1.0 },
            },
            UnitEntry {
                id: "pinch",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.308,
                },
            },
            UnitEntry {
                id: "smidgen",
                conversion: UnitConversion::Linear {
                    factor_to_si: 0.616,
                },
            },
            UnitEntry {
                id: "tablespoon_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 17.7582,
                },
            },
            UnitEntry {
                id: "tablespoon_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 14.7868,
                },
            },
            UnitEntry {
                id: "teaspoon_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 5.91939,
                },
            },
            UnitEntry {
                id: "teaspoon_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 4.92892,
                },
            },
            UnitEntry {
                id: "fluid_ounce_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 29.5735,
                },
            },
            UnitEntry {
                id: "fluid_ounce_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 28.4131,
                },
            },
            UnitEntry {
                id: "pint_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 473.176,
                },
            },
            UnitEntry {
                id: "pint_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 568.261,
                },
            },
            UnitEntry {
                id: "quart_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 946.353,
                },
            },
            UnitEntry {
                id: "quart_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1136.52,
                },
            },
            UnitEntry {
                id: "gallon_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 3785.41,
                },
            },
            UnitEntry {
                id: "gallon_uk",
                conversion: UnitConversion::Linear {
                    factor_to_si: 4546.09,
                },
            },
            UnitEntry {
                id: "liter",
                conversion: UnitConversion::Linear {
                    factor_to_si: 1000.0,
                },
            },
            UnitEntry {
                id: "drop",
                conversion: UnitConversion::Linear { factor_to_si: 0.05 },
            },
            UnitEntry {
                id: "bushel_us",
                conversion: UnitConversion::Linear {
                    factor_to_si: 35239.1,
                },
            },
            UnitEntry {
                id: "peck",
                conversion: UnitConversion::Linear {
                    factor_to_si: 8809.77,
                },
            },
        ],
    },
    CategoryTable {
        category_id: "temperature",
        units: &[
            // celsius: to_kelvin = x + 273.15
            UnitEntry {
                id: "celsius",
                conversion: UnitConversion::Affine {
                    scale: 1.0,
                    offset: 273.15,
                },
            },
            // delisle: to_kelvin = 373.15 - x * 2/3
            UnitEntry {
                id: "delisle",
                conversion: UnitConversion::Affine {
                    scale: -2.0 / 3.0,
                    offset: 373.15,
                },
            },
            // fahrenheit: to_kelvin = (x + 459.67) * 5/9
            UnitEntry {
                id: "fahrenheit",
                conversion: UnitConversion::Affine {
                    scale: 5.0 / 9.0,
                    offset: 459.67 * 5.0 / 9.0,
                },
            },
            // kelvin: to_kelvin = x
            UnitEntry {
                id: "kelvin",
                conversion: UnitConversion::Affine {
                    scale: 1.0,
                    offset: 0.0,
                },
            },
            // newton_temp: to_kelvin = x * 100/33 + 273.15
            UnitEntry {
                id: "newton_temp",
                conversion: UnitConversion::Affine {
                    scale: 100.0 / 33.0,
                    offset: 273.15,
                },
            },
            // planck_temp: to_kelvin = x * 1.417e32
            UnitEntry {
                id: "planck_temp",
                conversion: UnitConversion::Affine {
                    scale: 1.417e32,
                    offset: 0.0,
                },
            },
            // rankine: to_kelvin = x * 5/9
            UnitEntry {
                id: "rankine",
                conversion: UnitConversion::Affine {
                    scale: 5.0 / 9.0,
                    offset: 0.0,
                },
            },
            // reaumur: to_kelvin = x * 5/4 + 273.15
            UnitEntry {
                id: "reaumur",
                conversion: UnitConversion::Affine {
                    scale: 5.0 / 4.0,
                    offset: 273.15,
                },
            },
            // romer: to_kelvin = (x - 7.5) * 40/21 + 273.15
            UnitEntry {
                id: "romer",
                conversion: UnitConversion::Affine {
                    scale: 40.0 / 21.0,
                    offset: 273.15 - 7.5 * 40.0 / 21.0,
                },
            },
            // gas_mark: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).
            UnitEntry {
                id: "gas_mark",
                conversion: UnitConversion::Unconvertible,
            },
            // triple_point_water: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).
            UnitEntry {
                id: "triple_point_water",
                conversion: UnitConversion::Unconvertible,
            },
        ],
    },
    CategoryTable {
        category_id: "fuel_consumption",
        units: &[
            // km_per_liter: to_base = 100/x
            UnitEntry {
                id: "km_per_liter",
                conversion: UnitConversion::Reciprocal { k: 100.0 },
            },
            UnitEntry {
                id: "liter_per_100km",
                conversion: UnitConversion::Linear { factor_to_si: 1.0 },
            },
            // mile_per_liter: to_base = 100/1.60934/x
            UnitEntry {
                id: "mile_per_liter",
                conversion: UnitConversion::Reciprocal { k: 100.0 / 1.60934 },
            },
            // mpg_uk: to_base = 282.481/x
            UnitEntry {
                id: "mpg_uk",
                conversion: UnitConversion::Reciprocal { k: 282.481 },
            },
            // mpg_us: to_base = 235.215/x
            UnitEntry {
                id: "mpg_us",
                conversion: UnitConversion::Reciprocal { k: 235.215 },
            },
            // gallon_per_100mile_us: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).
            UnitEntry {
                id: "gallon_per_100mile_us",
                conversion: UnitConversion::Unconvertible,
            },
            // gallon_per_100mile_uk: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).
            UnitEntry {
                id: "gallon_per_100mile_uk",
                conversion: UnitConversion::Unconvertible,
            },
            // liter_per_km: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).
            UnitEntry {
                id: "liter_per_km",
                conversion: UnitConversion::Unconvertible,
            },
            // liter_per_mile: no factor_to_si and no override — no conversion data in the legacy catalog either (see packages/data/src/units/README.md).
            UnitEntry {
                id: "liter_per_mile",
                conversion: UnitConversion::Unconvertible,
            },
        ],
    },
];
