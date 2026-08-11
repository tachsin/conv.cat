//! Life-age (species years ⇄ human years) conversions — piecewise-interpolated curves for the
//! handful of well-studied pets, a simple lifespan-ratio model for everything else. Hand-ported,
//! not table-driven from `packages/data` like [`super::catalog`]'s categories, because this is
//! genuinely an algorithm (curve interpolation), not a flat factor table.
//!
//! Faithfully transcribed from `conv.cat_legacy/conv-rust/src-tauri/src/units/life_age.rs` (the
//! more complete of the two legacy implementations — the TS version in
//! `conv-next/lib/life-age.ts` is equivalent for the conversion math itself). All 47 species in
//! `packages/data/src/units/categories/life_age.json` are fully specified here — unlike
//! `temperature`/`fuel_consumption`, there are no honest gaps in this category.

use crate::{ConvertError, Format};

#[derive(Clone, Copy)]
struct Point {
    age: f64,
    human: f64,
}

enum Model {
    /// Human years, unchanged.
    Identity,
    /// Interpolates between known `(age, human)` knots; beyond the last knot, extrapolates
    /// linearly at `after_rate` human-years per species-year.
    Piecewise {
        curve: &'static [Point],
        after_rate: f64,
    },
    /// `human = age * (HUMAN_REF / max_years)` — a straight-line ratio against this species'
    /// typical lifespan. The catch-all for species without a published age-equivalence study.
    Lifespan { max_years: f64 },
}

/// The reference human lifespan the [`Model::Lifespan`] ratio is scaled against. Matches the
/// legacy `HUMAN_REF` constant exactly.
const HUMAN_REF: f64 = 80.0;

/// AVMA year 1–2 milestones (shared by every `Piecewise` model below) plus the AKC medium-dog
/// table for year 3+. Source: https://www.akc.org/expert-advice/health/how-to-calculate-dog-years-to-human-years/
const DOG_CURVE: [Point; 7] = [
    Point {
        age: 0.0,
        human: 0.0,
    },
    Point {
        age: 1.0,
        human: 15.0,
    },
    Point {
        age: 2.0,
        human: 24.0,
    },
    Point {
        age: 3.0,
        human: 28.0,
    },
    Point {
        age: 5.0,
        human: 37.0,
    },
    Point {
        age: 7.0,
        human: 47.0,
    },
    Point {
        age: 10.0,
        human: 61.0,
    },
];

/// AVMA / PetMD cat age chart. Source:
/// https://www.petmd.com/cat/general-health/how-old-is-my-cat-in-human-years
const CAT_CURVE: [Point; 4] = [
    Point {
        age: 0.0,
        human: 0.0,
    },
    Point {
        age: 0.5,
        human: 10.0,
    },
    Point {
        age: 1.0,
        human: 15.0,
    },
    Point {
        age: 2.0,
        human: 24.0,
    },
];

const RABBIT_CURVE: [Point; 3] = [
    Point {
        age: 0.0,
        human: 0.0,
    },
    Point {
        age: 1.0,
        human: 20.0,
    },
    Point {
        age: 2.0,
        human: 28.0,
    },
];

const HORSE_CURVE: [Point; 3] = [
    Point {
        age: 0.0,
        human: 0.0,
    },
    Point {
        age: 1.0,
        human: 6.5,
    },
    Point {
        age: 2.0,
        human: 13.0,
    },
];

fn model_for(species_id: &str) -> Option<Model> {
    match species_id {
        "human" => Some(Model::Identity),
        "dog" => Some(Model::Piecewise {
            curve: &DOG_CURVE,
            after_rate: 14.0 / 3.0,
        }),
        "cat" => Some(Model::Piecewise {
            curve: &CAT_CURVE,
            after_rate: 4.0,
        }),
        "rabbit" => Some(Model::Piecewise {
            curve: &RABBIT_CURVE,
            after_rate: 6.0,
        }),
        "horse" => Some(Model::Piecewise {
            curve: &HORSE_CURVE,
            after_rate: 2.5,
        }),
        "hamster" => Some(Model::Lifespan { max_years: 3.0 }),
        "guinea_pig" | "ferret" => Some(Model::Lifespan { max_years: 8.0 }),
        "chinchilla" => Some(Model::Lifespan { max_years: 15.0 }),
        "mouse" => Some(Model::Lifespan { max_years: 2.0 }),
        "rat" => Some(Model::Lifespan { max_years: 3.0 }),
        "cow" => Some(Model::Lifespan { max_years: 20.0 }),
        "pig" | "goat" => Some(Model::Lifespan { max_years: 15.0 }),
        "sheep" => Some(Model::Lifespan { max_years: 12.0 }),
        "elephant" => Some(Model::Lifespan { max_years: 70.0 }),
        "chicken" => Some(Model::Lifespan { max_years: 8.0 }),
        "duck" | "canary" => Some(Model::Lifespan { max_years: 10.0 }),
        "parrot_large" => Some(Model::Lifespan { max_years: 50.0 }),
        "parrot_small" => Some(Model::Lifespan { max_years: 15.0 }),
        "turtle" => Some(Model::Lifespan { max_years: 40.0 }),
        "tortoise" | "bonsai" => Some(Model::Lifespan { max_years: 100.0 }),
        "snake" => Some(Model::Lifespan { max_years: 20.0 }),
        "lizard" => Some(Model::Lifespan { max_years: 10.0 }),
        "gecko" => Some(Model::Lifespan { max_years: 15.0 }),
        "goldfish" => Some(Model::Lifespan { max_years: 10.0 }),
        "betta" => Some(Model::Lifespan { max_years: 3.0 }),
        "koi" => Some(Model::Lifespan { max_years: 35.0 }),
        "bee" => Some(Model::Lifespan { max_years: 1.0 }),
        "houseplant" => Some(Model::Lifespan { max_years: 25.0 }),
        "succulent" => Some(Model::Lifespan { max_years: 20.0 }),
        "oak_tree" => Some(Model::Lifespan { max_years: 300.0 }),
        "pine_tree" => Some(Model::Lifespan { max_years: 200.0 }),
        "maple_tree" => Some(Model::Lifespan { max_years: 150.0 }),
        "bamboo" => Some(Model::Lifespan { max_years: 60.0 }),
        "rose_bush" => Some(Model::Lifespan { max_years: 30.0 }),
        "fruit_tree" => Some(Model::Lifespan { max_years: 50.0 }),
        "tomato_plant" | "annual_flower" | "wheat_crop" => Some(Model::Lifespan { max_years: 1.0 }),
        "perennial_flower" => Some(Model::Lifespan { max_years: 8.0 }),
        "grass_lawn" => Some(Model::Lifespan { max_years: 5.0 }),
        "fern" => Some(Model::Lifespan { max_years: 15.0 }),
        "cactus" => Some(Model::Lifespan { max_years: 50.0 }),
        "orchid" => Some(Model::Lifespan { max_years: 20.0 }),
        _ => None,
    }
}

fn piecewise_to_human(age: f64, curve: &[Point], after_rate: f64) -> f64 {
    if age <= 0.0 || !age.is_finite() {
        return 0.0;
    }

    for index in 1..curve.len() {
        let prev = curve[index - 1];
        let next = curve[index];
        if age <= next.age {
            let span = next.age - prev.age;
            if span <= 0.0 {
                return next.human;
            }
            let ratio = (age - prev.age) / span;
            return prev.human + ratio * (next.human - prev.human);
        }
    }

    let last = curve[curve.len() - 1];
    last.human + (age - last.age) * after_rate
}

fn piecewise_from_human(human: f64, curve: &[Point], after_rate: f64) -> f64 {
    if human <= 0.0 || !human.is_finite() {
        return 0.0;
    }

    for index in 1..curve.len() {
        let prev = curve[index - 1];
        let next = curve[index];
        if human <= next.human {
            let span = next.human - prev.human;
            if span <= 0.0 {
                return next.age;
            }
            let ratio = (human - prev.human) / span;
            return prev.age + ratio * (next.age - prev.age);
        }
    }

    let last = curve[curve.len() - 1];
    last.age + (human - last.human) / after_rate
}

fn species_to_human_years(species_id: &str, age: f64) -> Option<f64> {
    let model = model_for(species_id)?;
    match model {
        Model::Identity => Some(age),
        Model::Piecewise { curve, after_rate } => Some(piecewise_to_human(age, curve, after_rate)),
        Model::Lifespan { max_years } => Some(age * (HUMAN_REF / max_years)),
    }
}

fn human_years_to_species(species_id: &str, human_years: f64) -> Option<f64> {
    let model = model_for(species_id)?;
    match model {
        Model::Identity => Some(human_years),
        Model::Piecewise { curve, after_rate } => {
            Some(piecewise_from_human(human_years, curve, after_rate))
        }
        Model::Lifespan { max_years } => Some(human_years * (max_years / HUMAN_REF)),
    }
}

/// Converts `value` species-years from `from_id` to `to_id`, via human years as the shared scale
/// — matches `conv_life_age::convert_life_age`'s two-hop shape exactly.
pub fn convert(
    from_id: &str,
    to_id: &str,
    value: f64,
    format: Format,
) -> Result<f64, ConvertError> {
    if from_id == to_id {
        return Ok(value);
    }
    if !value.is_finite() {
        return Err(ConvertError::MalformedInput { format });
    }

    let unsupported = || ConvertError::UnsupportedFeature {
        format,
        feature: "unknown_unit",
    };

    let human = species_to_human_years(from_id, value).ok_or_else(unsupported)?;
    let result = human_years_to_species(to_id, human).ok_or_else(unsupported)?;

    if !result.is_finite() {
        return Err(ConvertError::MalformedInput { format });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dog_at_a_curve_knot_matches_the_table_exactly() {
        // age 3 is an exact DOG_CURVE knot (28.0 human years) — no interpolation involved, so
        // this should be bit-exact, not just close.
        let result = convert("dog", "human", 3.0, Format::PlainText).unwrap();
        assert_eq!(result, 28.0);
    }

    #[test]
    fn dog_interpolates_between_knots() {
        // Halfway between (1, 15) and (2, 24) -> 19.5.
        let result = convert("dog", "human", 1.5, Format::PlainText).unwrap();
        assert_eq!(result, 19.5);
    }

    #[test]
    fn dog_extrapolates_past_the_last_knot_at_after_rate() {
        // Last knot (10, 61), after_rate 14/3 per year.
        let result = convert("dog", "human", 11.0, Format::PlainText).unwrap();
        assert!((result - (61.0 + 14.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn lifespan_model_is_a_straight_ratio() {
        // hamster: max_years 3, HUMAN_REF 80 -> 1.5 * 80/3 = 40.
        let result = convert("hamster", "human", 1.5, Format::PlainText).unwrap();
        assert_eq!(result, 40.0);
    }

    #[test]
    fn round_trip_through_human_is_the_identity() {
        let human = convert("cat", "human", 4.0, Format::PlainText).unwrap();
        let back = convert("human", "cat", human, Format::PlainText).unwrap();
        assert!((back - 4.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_species_is_unsupported_feature() {
        let err = convert("dragon", "human", 1.0, Format::PlainText).unwrap_err();
        assert!(matches!(err, ConvertError::UnsupportedFeature { .. }));
    }
}
