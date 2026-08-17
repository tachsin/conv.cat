//! The golden-file conformance suite: the executable spec every `conv-core` converter must
//! satisfy. See `tests/fixtures/README.md` for the corpus layout and
//! `docs/ARCHITECTURE.md#the-conformance-suite` for why it exists.
//!
//! One `#[test]` per fixture case, grouped by category/format — this mirrors the fixture tree on
//! disk and matches the pattern documented in
//! `docs/adding-a-format.md#step-4--golden-file-tests`. When you add a fixture, add its files to
//! the `assert_no_stray_files` manifest at the bottom of this file too: that test fails the build
//! if a fixture exists that no test actually exercises.

mod support;

use conv_core::formats::units::generated;
use conv_core::{ConvertOptions, Format};

// ─── image / bmp, image / qoi ─────────────────────────────────────────────────
//
// The first file-shaped category: BMP ⇄ QOI, both directions, via a shared 4×4 checkerboard
// fixture (white/black alternating pixels — enough to exercise QOI's index-cache, diff, and
// run-length chunk kinds without needing a large image). See `crates/conv-core/src/formats/image`
// module docs for why these two formats specifically. Both are deterministic/lossless, so these
// are byte-exact goldens like `plain_text` above, not the tolerance-band style `units` uses.

#[test]
fn image_bmp_to_qoi_checkerboard() {
    support::run_golden_case(
        "image/qoi/checker_4x4.bmp",
        "image/qoi/checker_4x4.qoi",
        Format::Bmp,
        Format::Qoi,
    );
}

#[test]
fn image_qoi_to_bmp_checkerboard() {
    support::run_golden_case(
        "image/bmp/checker_4x4.qoi",
        "image/bmp/checker_4x4.bmp",
        Format::Qoi,
        Format::Bmp,
    );
}

#[test]
fn image_bmp_truncated_pixel_data_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/qoi/truncated.bmp.bad",
        Format::Bmp,
        Format::Qoi,
    );
}

#[test]
fn image_qoi_bad_magic_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/bmp/bad_magic.qoi.bad",
        Format::Qoi,
        Format::Bmp,
    );
}

#[test]
fn image_qoi_trailing_data_after_end_marker_is_a_typed_error() {
    // A stream that decodes the declared pixel count fine but keeps going past where the
    // mandatory end marker says it must stop — the trailing-data check in `qoi::decode` exists
    // specifically to catch this, not just the per-chunk bounds checks.
    support::assert_malformed_produces_typed_error(
        "image/bmp/trailing_data.qoi.bad",
        Format::Qoi,
        Format::Bmp,
    );
}

#[test]
fn image_qoi_corrupted_end_marker_is_a_typed_error() {
    // Right length, wrong terminator bytes (last byte flipped from 0x01 to 0x00) — decodes the
    // right pixel count but doesn't end with the spec's mandatory 7-zero-bytes-then-0x01 marker.
    support::assert_malformed_produces_typed_error(
        "image/bmp/bad_end_marker.qoi.bad",
        Format::Qoi,
        Format::Bmp,
    );
}

// ─── image / png ───────────────────────────────────────────────────────────
//
// PNG joins the BMP/QOI raster hub (see `crates/conv-core/src/formats/image/png.rs` module docs
// for why PNG specifically, and why hand-rolling a real DEFLATE codec was worth it over pulling in
// a dependency). Same shared checkerboard fixture as BMP/QOI above, both directions, plus the
// malformed-input corpus. The decoder's harder cases (real dynamic-Huffman compression, all five
// PNG scanline filter types against an independently-built PNG file, not just this crate's own
// encoder's output) are covered as Rust unit tests in `png.rs` itself, not here — this suite
// proves the *public* `conv_core::convert` API end to end, not the codec's internals.

#[test]
fn image_bmp_to_png_checkerboard() {
    support::run_golden_case(
        "image/png/checker_4x4.bmp",
        "image/png/checker_4x4.png",
        Format::Bmp,
        Format::Png,
    );
}

#[test]
fn image_qoi_to_png_checkerboard() {
    support::run_golden_case(
        "image/png/checker_4x4.qoi",
        "image/png/checker_4x4.png",
        Format::Qoi,
        Format::Png,
    );
}

#[test]
fn image_png_to_bmp_checkerboard() {
    support::run_golden_case(
        "image/bmp/checker_4x4.png",
        "image/bmp/checker_4x4.bmp",
        Format::Png,
        Format::Bmp,
    );
}

#[test]
fn image_png_to_qoi_checkerboard() {
    support::run_golden_case(
        "image/qoi/checker_4x4.png",
        "image/qoi/checker_4x4.qoi",
        Format::Png,
        Format::Qoi,
    );
}

#[test]
fn image_png_bad_signature_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/qoi/bad_signature.png.bad",
        Format::Png,
        Format::Qoi,
    );
}

#[test]
fn image_png_truncated_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/bmp/truncated.png.bad",
        Format::Png,
        Format::Bmp,
    );
}

#[test]
fn image_png_corrupted_chunk_crc_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/bmp/bad_crc.png.bad",
        Format::Png,
        Format::Bmp,
    );
}

// ─── image / ico ───────────────────────────────────────────────────────────
//
// ICO joins the raster hub last — see `crates/conv-core/src/formats/image/ico.rs` module docs
// for why it's a cheap addition once PNG exists (a directory wrapping PNG entries, not a new
// codec). Same shared checkerboard fixture as BMP/QOI/PNG above. The decoder's harder case (a
// real 32-bit raw-DIB entry from an independent encoder, since this crate's own encoder only
// ever emits PNG-compressed entries) is a Rust unit test in `ico.rs` itself, same split PNG's
// suite uses for its own harder decoder cases.

#[test]
fn image_bmp_to_ico_checkerboard() {
    support::run_golden_case(
        "image/ico/checker_4x4.bmp",
        "image/ico/checker_4x4.ico",
        Format::Bmp,
        Format::Ico,
    );
}

#[test]
fn image_qoi_to_ico_checkerboard() {
    support::run_golden_case(
        "image/ico/checker_4x4.qoi",
        "image/ico/checker_4x4.ico",
        Format::Qoi,
        Format::Ico,
    );
}

#[test]
fn image_png_to_ico_checkerboard() {
    support::run_golden_case(
        "image/ico/checker_4x4.png",
        "image/ico/checker_4x4.ico",
        Format::Png,
        Format::Ico,
    );
}

#[test]
fn image_ico_to_bmp_checkerboard() {
    support::run_golden_case(
        "image/bmp/checker_4x4.ico",
        "image/bmp/checker_4x4.bmp",
        Format::Ico,
        Format::Bmp,
    );
}

#[test]
fn image_ico_to_qoi_checkerboard() {
    support::run_golden_case(
        "image/qoi/checker_4x4.ico",
        "image/qoi/checker_4x4.qoi",
        Format::Ico,
        Format::Qoi,
    );
}

#[test]
fn image_ico_to_png_checkerboard() {
    support::run_golden_case(
        "image/png/checker_4x4.ico",
        "image/png/checker_4x4.png",
        Format::Ico,
        Format::Png,
    );
}

#[test]
fn image_ico_zero_entries_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/bmp/zero_entries.ico.bad",
        Format::Ico,
        Format::Bmp,
    );
}

#[test]
fn image_ico_truncated_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/qoi/truncated.ico.bad",
        Format::Ico,
        Format::Qoi,
    );
}

#[test]
fn image_ico_cursor_resource_type_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "image/png/cursor_type.ico.bad",
        Format::Ico,
        Format::Png,
    );
}

// ─── text / plain_text ──────────────────────────────────────────────────────
//
// `IdentityConverter` (PlainText -> PlainText) is a passthrough, so every golden file here is
// byte-identical to its input by construction. That's deliberate: these cases exist to prove the
// fixture-driven harness itself — reading real files off disk, dispatching through the public
// `conv_core::convert` API, comparing byte-for-byte, end to end — so the next contributor adding
// a real format (image, units, text — see the backlog) has a proven pattern to copy, not just
// documentation to trust.

#[test]
fn plain_text_identity_hello() {
    support::run_golden_case(
        "text/plain_text/hello.input.txt",
        "text/plain_text/hello.golden.txt",
        Format::PlainText,
        Format::PlainText,
    );
}

#[test]
fn plain_text_identity_empty_input() {
    support::run_golden_case(
        "text/plain_text/empty.input.txt",
        "text/plain_text/empty.golden.txt",
        Format::PlainText,
        Format::PlainText,
    );
}

#[test]
fn plain_text_identity_non_utf8_bytes_pass_through_unchanged() {
    // Guards against a future edit accidentally adding UTF-8 validation to `IdentityConverter` —
    // conv-core converters operate on `&[u8]`, not `&str`, and this format's contract is
    // byte-transparent, not text-transparent. This fixture intentionally contains invalid UTF-8.
    support::run_golden_case(
        "text/plain_text/non_utf8_bytes.input.txt",
        "text/plain_text/non_utf8_bytes.golden.txt",
        Format::PlainText,
        Format::PlainText,
    );
}

// ─── units ───────────────────────────────────────────────────────────────────
//
// See `crates/conv-core/src/formats/units/mod.rs` for the wire protocol
// (`"<from_unit_id>|<to_unit_id>|<value>"` as UTF-8 text) and why units are modeled as one
// `Format` self-pair per *category* rather than per unit. Golden pairs below are picked to prove
// each of the five conversion models actually works — in particular the two the legacy app never
// implemented despite shipping catalog data for them: temperature's **offset**, not just scale
// (`UnitConversion::Affine`), and fuel_consumption's **reciprocal** math
// (`UnitConversion::Reciprocal`). See `packages/data/src/units/README.md` for that history.

#[test]
fn units_length_meter_to_foot() {
    support::run_golden_case(
        "units/length/meter_to_foot.input.txt",
        "units/length/meter_to_foot.golden.txt",
        Format::UnitsLength,
        Format::UnitsLength,
    );
}

#[test]
fn units_length_foot_to_meter() {
    support::run_golden_case(
        "units/length/foot_to_meter.input.txt",
        "units/length/foot_to_meter.golden.txt",
        Format::UnitsLength,
        Format::UnitsLength,
    );
}

#[test]
fn units_length_round_trip_stays_within_tolerance_of_the_original_value() {
    // Not a golden-file case (the point is the *relative* error, not a specific byte-exact
    // output) — see `formats::units` module docs' "Precision" section for the tolerance
    // rationale. meter -> foot -> meter through two independent Linear hops.
    let original = 1.234_f64;

    let to_feet = format!("meter|foot|{original}");
    let feet_bytes = conv_core::convert(
        to_feet.as_bytes(),
        Format::UnitsLength,
        Format::UnitsLength,
        &ConvertOptions::default(),
    )
    .unwrap();
    let feet_value: f64 = String::from_utf8(feet_bytes).unwrap().parse().unwrap();

    let back = format!("foot|meter|{feet_value}");
    let back_bytes = conv_core::convert(
        back.as_bytes(),
        Format::UnitsLength,
        Format::UnitsLength,
        &ConvertOptions::default(),
    )
    .unwrap();
    let back_value: f64 = String::from_utf8(back_bytes).unwrap().parse().unwrap();

    let relative_error = ((back_value - original) / original).abs();
    assert!(
        relative_error < 1e-9,
        "meter -> foot -> meter drifted too far: {original} -> {feet_value} -> {back_value} \
         (relative error {relative_error})"
    );
}

#[test]
fn units_temperature_fahrenheit_to_celsius_freezing_point() {
    support::run_golden_case(
        "units/temperature/fahrenheit_to_celsius.input.txt",
        "units/temperature/fahrenheit_to_celsius.golden.txt",
        Format::UnitsTemperature,
        Format::UnitsTemperature,
    );
}

#[test]
fn units_temperature_fahrenheit_to_celsius_boiling_point() {
    support::run_golden_case(
        "units/temperature/fahrenheit_212_to_celsius.input.txt",
        "units/temperature/fahrenheit_212_to_celsius.golden.txt",
        Format::UnitsTemperature,
        Format::UnitsTemperature,
    );
}

#[test]
fn units_temperature_celsius_to_kelvin_proves_the_offset() {
    // 0 -> 273.15, not 0 -> 0: this is the specific case that makes temperature the "offset, not
    // just scale" example the units vertical-slice ticket asked to get right, and which the
    // legacy app never actually implemented (see packages/data/src/units/README.md).
    support::run_golden_case(
        "units/temperature/celsius_to_kelvin.input.txt",
        "units/temperature/celsius_to_kelvin.golden.txt",
        Format::UnitsTemperature,
        Format::UnitsTemperature,
    );
}

#[test]
fn units_fuel_consumption_mpg_us_to_liter_per_100km_proves_the_reciprocal() {
    // mpg_us's `k = 235.215`, so `mpg_us|liter_per_100km|235.215` should land exactly on `1` —
    // proves `UnitConversion::Reciprocal`'s `k / x` math, not just linear scaling.
    support::run_golden_case(
        "units/fuel_consumption/mpg_us_to_l100km.input.txt",
        "units/fuel_consumption/mpg_us_to_l100km.golden.txt",
        Format::UnitsFuelConsumption,
        Format::UnitsFuelConsumption,
    );
}

#[test]
fn units_life_age_dog_at_an_exact_curve_knot() {
    // age 3 is an exact DOG_CURVE knot (28 human years) — see
    // `crates/conv-core/src/formats/units/life_age.rs`.
    support::run_golden_case(
        "units/life_age/dog_to_human.input.txt",
        "units/life_age/dog_to_human.golden.txt",
        Format::UnitsLifeAge,
        Format::UnitsLifeAge,
    );
}

#[test]
fn units_life_age_hamster_lifespan_ratio() {
    support::run_golden_case(
        "units/life_age/hamster_to_human.input.txt",
        "units/life_age/hamster_to_human.golden.txt",
        Format::UnitsLifeAge,
        Format::UnitsLifeAge,
    );
}

#[test]
fn units_clothing_size_numeric_chart_lookup_to_letter() {
    support::run_golden_case(
        "units/clothing_size/numeric_to_letter.input.txt",
        "units/clothing_size/numeric_to_letter.golden.txt",
        Format::UnitsClothingSize,
        Format::UnitsClothingSize,
    );
}

#[test]
fn units_clothing_size_letter_label_input_to_numeric() {
    // Proves the wire payload's value field actually round-trips *text*, not just numbers — the
    // one category in scope where that matters.
    support::run_golden_case(
        "units/clothing_size/letter_to_numeric.input.txt",
        "units/clothing_size/letter_to_numeric.golden.txt",
        Format::UnitsClothingSize,
        Format::UnitsClothingSize,
    );
}

#[test]
fn unit_round_trips_stay_within_tolerance_across_every_generic_model_category() {
    // The full-breadth proof that "every unit in scope converts correctly", per this ticket's
    // acceptance criteria — every unit in every table-driven category (`length`, `mass`,
    // `volume`, `cooking`, `temperature`, `fuel_consumption`) does a self round trip
    // (`unit -> SI -> unit`) and must land within 1e-9 relative error of where it started.
    // `life_age`/`clothing_size` aren't table-driven the same way and have their own targeted
    // tests above. See `formats::units` module docs' "Precision" section for why 1e-9.
    const PROBE_VALUES: &[f64] = &[1.0, 42.5, -7.25, 1000.0, 0.001];

    let mut checked = 0usize;
    for table in generated::CATEGORIES {
        for unit in table.units {
            if matches!(
                unit.conversion,
                conv_core::formats::units::catalog::UnitConversion::Unconvertible
            ) {
                continue; // no conversion data to round-trip — see the category's honest gaps.
            }

            for &value in PROBE_VALUES {
                // Reciprocal units are undefined at 0 and blow up near it — skip probe values
                // that would divide by (near-)zero for this specific unit's `k`, rather than
                // asserting a meaningless huge relative error.
                if let conv_core::formats::units::catalog::UnitConversion::Reciprocal { .. } =
                    unit.conversion
                {
                    if value == 0.0 {
                        continue;
                    }
                }

                let payload = format!("{}|{}|{value}", unit.id, unit.id);
                let format = format_for_category(table.category_id);
                let output = conv_core::convert(
                    payload.as_bytes(),
                    format,
                    format,
                    &ConvertOptions::default(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "{}/{} round trip at {value} failed: {err:?}",
                        table.category_id, unit.id
                    )
                });
                let result: f64 = String::from_utf8(output).unwrap().parse().unwrap();
                assert_eq!(
                    result, value,
                    "{}/{} identity self-conversion should be exact (from_unit == to_unit \
                     short-circuits before any math runs)",
                    table.category_id, unit.id
                );

                // Now the real round trip: unit -> SI -> unit, via a *different* probe pairing so
                // real arithmetic actually runs (from_unit == to_unit above only proved the
                // short-circuit path, not the math).
                let si_unit = table
                    .units
                    .iter()
                    .find(|u| {
                        !matches!(
                            u.conversion,
                            conv_core::formats::units::catalog::UnitConversion::Unconvertible
                        )
                    })
                    .expect("every in-scope category has at least one convertible unit");
                if si_unit.id == unit.id {
                    continue; // nothing to compare against in this category
                }

                let to_si_payload = format!("{}|{}|{value}", unit.id, si_unit.id);
                let si_output = conv_core::convert(
                    to_si_payload.as_bytes(),
                    format,
                    format,
                    &ConvertOptions::default(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "{}/{} -> {} failed at {value}: {err:?}",
                        table.category_id, unit.id, si_unit.id
                    )
                });
                let si_value: f64 = String::from_utf8(si_output).unwrap().parse().unwrap();
                if si_value == 0.0 {
                    continue; // can't compute a meaningful relative error against zero
                }

                let back_payload = format!("{}|{}|{si_value}", si_unit.id, unit.id);
                let back_output = conv_core::convert(
                    back_payload.as_bytes(),
                    format,
                    format,
                    &ConvertOptions::default(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "{}/{} <- {} failed at {si_value}: {err:?}",
                        table.category_id, unit.id, si_unit.id
                    )
                });
                let back_value: f64 = String::from_utf8(back_output).unwrap().parse().unwrap();

                let relative_error = ((back_value - value) / value).abs();
                assert!(
                    relative_error < 1e-9,
                    "{}/{} round trip via {} drifted too far: {value} -> {si_value} -> \
                     {back_value} (relative error {relative_error})",
                    table.category_id,
                    unit.id,
                    si_unit.id
                );
                checked += 1;
            }
        }
    }

    assert!(
        checked > 100,
        "expected to have exercised well over 100 unit round trips, only checked {checked} — \
         did the generated catalog shrink unexpectedly?"
    );
}

fn format_for_category(category_id: &str) -> Format {
    match category_id {
        "length" => Format::UnitsLength,
        "mass" => Format::UnitsMass,
        "volume" => Format::UnitsVolume,
        "cooking" => Format::UnitsCooking,
        "temperature" => Format::UnitsTemperature,
        "fuel_consumption" => Format::UnitsFuelConsumption,
        other => panic!("unexpected generated category id in golden.rs test: {other}"),
    }
}

// ─── malformed-input / unsupported-feature corpus ────────────────────────────
//
// `IdentityConverter` (above) performs no parsing, so it has no malformed-input failure mode —
// units is the first converter in this crate that actually does. Every `.bad` fixture here must
// make the converter return a typed `ConvertError`, never panic or hang — proven per-fixture via
// `support::assert_malformed_produces_typed_error`, which is deliberately agnostic about *which*
// `ConvertError` variant comes back (`MalformedInput` for a bad payload shape,
// `UnsupportedFeature` for an honest catalog gap — both are "typed error, not a crash").
//
// The malformed-input harness itself (panic containment + hang timeout) is proven independently
// of any real parser in `tests/golden_harness_selftest.rs`, using test-double converters.

#[test]
fn units_wrong_field_count_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "units/length/wrong_field_count.bad",
        Format::UnitsLength,
        Format::UnitsLength,
    );
}

#[test]
fn units_non_numeric_value_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "units/length/non_numeric_value.bad",
        Format::UnitsLength,
        Format::UnitsLength,
    );
}

#[test]
fn units_temperature_honest_gap_unit_is_a_typed_error_not_a_fabricated_result() {
    // gas_mark has no to_kelvin/from_kelvin formula in the legacy catalog either — see
    // packages/data/src/units/README.md. Must return UnsupportedFeature, never a made-up number.
    support::assert_malformed_produces_typed_error(
        "units/temperature/gas_mark_gap.bad",
        Format::UnitsTemperature,
        Format::UnitsTemperature,
    );
}

#[test]
fn units_fuel_consumption_honest_gap_unit_is_a_typed_error() {
    support::assert_malformed_produces_typed_error(
        "units/fuel_consumption/undefined_unit_gap.bad",
        Format::UnitsFuelConsumption,
        Format::UnitsFuelConsumption,
    );
}

#[test]
fn units_clothing_size_kids_sizing_honest_gap_is_a_typed_error() {
    // The legacy chart-based logic has no data for kids' sizing — see
    // packages/data/src/units/README.md.
    support::assert_malformed_produces_typed_error(
        "units/clothing_size/kids_gap.bad",
        Format::UnitsClothingSize,
        Format::UnitsClothingSize,
    );
}

// ─── corpus hygiene ──────────────────────────────────────────────────────────

#[test]
fn fixtures_tree_has_no_stray_or_orphaned_files() {
    support::assert_no_stray_files(&[
        "README.md",
        "image/qoi/checker_4x4.bmp",
        "image/qoi/checker_4x4.qoi",
        "image/qoi/truncated.bmp.bad",
        "image/bmp/checker_4x4.qoi",
        "image/bmp/checker_4x4.bmp",
        "image/bmp/bad_magic.qoi.bad",
        "image/bmp/trailing_data.qoi.bad",
        "image/bmp/bad_end_marker.qoi.bad",
        "image/qoi/checker_4x4.png",
        "image/qoi/bad_signature.png.bad",
        "image/bmp/checker_4x4.png",
        "image/bmp/truncated.png.bad",
        "image/bmp/bad_crc.png.bad",
        "image/png/checker_4x4.bmp",
        "image/png/checker_4x4.qoi",
        "image/png/checker_4x4.png",
        "image/ico/checker_4x4.bmp",
        "image/ico/checker_4x4.qoi",
        "image/ico/checker_4x4.png",
        "image/ico/checker_4x4.ico",
        "image/bmp/checker_4x4.ico",
        "image/qoi/checker_4x4.ico",
        "image/png/checker_4x4.ico",
        "image/bmp/zero_entries.ico.bad",
        "image/qoi/truncated.ico.bad",
        "image/png/cursor_type.ico.bad",
        "text/plain_text/hello.input.txt",
        "text/plain_text/hello.golden.txt",
        "text/plain_text/empty.input.txt",
        "text/plain_text/empty.golden.txt",
        "text/plain_text/non_utf8_bytes.input.txt",
        "text/plain_text/non_utf8_bytes.golden.txt",
        "units/length/meter_to_foot.input.txt",
        "units/length/meter_to_foot.golden.txt",
        "units/length/foot_to_meter.input.txt",
        "units/length/foot_to_meter.golden.txt",
        "units/length/wrong_field_count.bad",
        "units/length/non_numeric_value.bad",
        "units/temperature/fahrenheit_to_celsius.input.txt",
        "units/temperature/fahrenheit_to_celsius.golden.txt",
        "units/temperature/fahrenheit_212_to_celsius.input.txt",
        "units/temperature/fahrenheit_212_to_celsius.golden.txt",
        "units/temperature/celsius_to_kelvin.input.txt",
        "units/temperature/celsius_to_kelvin.golden.txt",
        "units/temperature/gas_mark_gap.bad",
        "units/fuel_consumption/mpg_us_to_l100km.input.txt",
        "units/fuel_consumption/mpg_us_to_l100km.golden.txt",
        "units/fuel_consumption/undefined_unit_gap.bad",
        "units/life_age/dog_to_human.input.txt",
        "units/life_age/dog_to_human.golden.txt",
        "units/life_age/hamster_to_human.input.txt",
        "units/life_age/hamster_to_human.golden.txt",
        "units/clothing_size/numeric_to_letter.input.txt",
        "units/clothing_size/numeric_to_letter.golden.txt",
        "units/clothing_size/letter_to_numeric.input.txt",
        "units/clothing_size/letter_to_numeric.golden.txt",
        "units/clothing_size/kids_gap.bad",
    ]);
}
