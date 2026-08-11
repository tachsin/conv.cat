//! Typed JS errors.
//!
//! `conv-core::ConvertError` is already typed (no English prose as the primary payload — see its
//! own docs). This module preserves that at the JS boundary instead of collapsing it into a
//! thrown string: every error surfaced to `packages/engine` is a plain object shaped
//! `{ kind, message, ...details }`, `kind` being a stable identifier the TS layer switches on and
//! `message` a developer-facing (not localized, not shown to end users) fallback for logs. The UI
//! is still where a `kind` becomes a localized string — same rule as the native path, just
//! crossing one more boundary. See `docs/ARCHITECTURE.md`'s "Typed errors and identifiers, not
//! human strings" section.

use conv_core::ConvertError;
use js_sys::{Object, Reflect};
use wasm_bindgen::JsValue;

/// Builds the `{ kind, message, ...details }` object for a [`ConvertError`] coming back from
/// `conv-core`. `kind` values are documented in `packages/engine`'s `ConvertErrorKind` union —
/// keep the two in sync when either changes.
pub(crate) fn from_convert_error(err: ConvertError) -> JsValue {
    let (kind, fields): (&str, Vec<(&str, JsValue)>) = match &err {
        ConvertError::UnsupportedPair { from, to } => (
            "unsupported_pair",
            vec![("from", from.id().into()), ("to", to.id().into())],
        ),
        ConvertError::MalformedInput { format } => {
            ("malformed_input", vec![("format", format.id().into())])
        }
        ConvertError::UnsupportedFeature { format, feature } => (
            "unsupported_feature",
            vec![
                ("format", format.id().into()),
                ("feature", (*feature).into()),
            ],
        ),
        ConvertError::SizeLimitExceeded { limit, actual } => (
            "size_limit_exceeded",
            vec![
                ("limit", (*limit as f64).into()),
                ("actual", (*actual as f64).into()),
            ],
        ),
        ConvertError::Cancelled => ("cancelled", vec![]),
        ConvertError::Internal { detail } => ("internal", vec![("detail", (*detail).into())]),
        // ConvertError is #[non_exhaustive]: conv-core can add a variant this crate hasn't been
        // updated for yet. Fail into a generic-but-still-typed shape rather than a compile error
        // or a panic, so an out-of-date conv-wasm build degrades gracefully instead of crashing.
        _ => ("unknown", vec![]),
    };
    build(kind, &err.to_string(), fields)
}

/// A binding-level error: something this crate rejected before ever calling into `conv-core`
/// (an unrecognized format id, an input over the hard memory ceiling). Same `{ kind, message,
/// ...details }` shape as [`from_convert_error`] so `packages/engine` has exactly one error
/// object shape to handle regardless of which side rejected the request.
pub(crate) fn binding_error(kind: &str, message: String, fields: Vec<(&str, JsValue)>) -> JsValue {
    build(kind, &message, fields)
}

fn build(kind: &str, message: &str, fields: Vec<(&str, JsValue)>) -> JsValue {
    let obj = Object::new();
    // Reflect::set on a freshly-created Object only fails for a non-extensible target, which a
    // bare `Object::new()` never is — the errors are discarded rather than propagated because
    // there is no meaningful way to fail out of building an error object itself.
    let _ = Reflect::set(&obj, &"kind".into(), &kind.into());
    let _ = Reflect::set(&obj, &"message".into(), &message.into());
    for (key, value) in fields {
        let _ = Reflect::set(&obj, &key.into(), &value);
    }
    obj.into()
}
