//! Bridges between `conv-core`'s [`Format`] enum and the string ids JS actually sends/receives.
//!
//! `Format` is `#[non_exhaustive]`, so this module can't derive a JS mapping generically — it
//! hand-maintains a list, same as `conv-core::registry`'s own `match` arms on `Format`. Every
//! format-by-format ticket that adds a `Format` variant must add it here too, or the new format
//! is invisible to JS even once `conv-core` can convert it. `cargo test -p conv-wasm` has a test
//! that would need updating alongside, as a reminder.

use conv_core::Format;
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

/// Every [`Format`] this binding knows how to expose to JS, in the order [`supported_formats`]
/// returns them. Single source of truth for [`parse_format`] and [`supported_formats`] so the
/// two can never drift apart.
const KNOWN_FORMATS: &[Format] = &[
    Format::PlainText,
    Format::Bmp,
    Format::Qoi,
    Format::Png,
    Format::Ico,
    Format::Webp,
    Format::UnitsLength,
    Format::UnitsMass,
    Format::UnitsVolume,
    Format::UnitsCooking,
    Format::UnitsTemperature,
    Format::UnitsFuelConsumption,
    Format::UnitsLifeAge,
    Format::UnitsClothingSize,
];

/// Parses a JS-supplied format id (e.g. `"plain_text"`, from [`Format::id`]) back into a
/// [`Format`]. Returns `None` for an id this binding doesn't recognize — either a typo, or a
/// `conv-core` format this crate hasn't been updated to expose yet.
pub fn parse_format(id: &str) -> Option<Format> {
    KNOWN_FORMATS.iter().copied().find(|f| f.id() == id)
}

/// Builds the plain JS object `packages/engine` reads a [`Format`]'s metadata from:
/// `{ id, category, mime, extensions, canRead, canWrite }`. Field names are camelCase to match
/// JS/TS convention at this boundary — everywhere on the Rust side stays snake_case.
fn format_to_js(format: Format) -> JsValue {
    let obj = Object::new();
    let category = match format.category() {
        conv_core::Category::Image => "image",
        conv_core::Category::Text => "text",
        conv_core::Category::Units => "units",
        // `Category` is `#[non_exhaustive]` too; a category this binding doesn't recognize yet
        // still gets a stable (if generic) label instead of panicking.
        _ => "unknown",
    };
    let extensions = Array::new();
    for ext in format.extensions() {
        extensions.push(&JsValue::from_str(ext));
    }
    let _ = Reflect::set(&obj, &"id".into(), &format.id().into());
    let _ = Reflect::set(&obj, &"category".into(), &category.into());
    let _ = Reflect::set(&obj, &"mime".into(), &format.mime().into());
    let _ = Reflect::set(&obj, &"extensions".into(), &extensions);
    let _ = Reflect::set(&obj, &"canRead".into(), &format.can_read().into());
    let _ = Reflect::set(&obj, &"canWrite".into(), &format.can_write().into());
    obj.into()
}

/// Every format this build can convert, as JS objects — see `format_to_js` above for the shape.
/// `packages/engine` uses this instead of hardcoding format ids, so the catalog only needs to be
/// correct in one place (this module) as new formats land.
#[wasm_bindgen(js_name = supportedFormats)]
pub fn supported_formats() -> Vec<JsValue> {
    KNOWN_FORMATS.iter().copied().map(format_to_js).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_format_round_trips_through_its_id() {
        for format in KNOWN_FORMATS {
            assert_eq!(parse_format(format.id()), Some(*format));
        }
    }

    #[test]
    fn unknown_id_parses_to_none() {
        assert_eq!(parse_format("not-a-real-format"), None);
    }
}
