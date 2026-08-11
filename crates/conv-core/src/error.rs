//! Typed errors returned by this crate.
//!
//! `conv-core` never panics on malformed or hostile input (see the crate-level docs), and it
//! never returns an English sentence as *the* representation of an error either: every variant
//! here carries typed data (a [`Format`], a short static identifier) that the UI layer localizes.
//! See `docs/ARCHITECTURE.md`'s "Typed errors and identifiers, not human strings" section for
//! why. [`std::fmt::Display`] is still implemented, for logs and `?`-propagation in non-UI
//! contexts (a future CLI, test failure output) — nothing in this crate calls it to build
//! user-facing text, and UI code should match on the variant instead of parsing this string.

use std::fmt;

use crate::registry::Format;

/// Everything that can go wrong converting one [`Format`] to another.
///
/// Marked `#[non_exhaustive]`: every format-by-format ticket after this one is expected to add
/// new failure modes (a timeout, a resource-limit variant other than size), and an enum-level
/// addition would otherwise be a breaking change for every downstream `match`. Match with a
/// wildcard arm, or use `matches!`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// No registered [`crate::Converter`] handles this `from -> to` pair.
    ///
    /// This is the normal, expected outcome of asking for a conversion nobody has implemented
    /// yet, not a bug — [`crate::convert`] returns it instead of panicking or guessing.
    UnsupportedPair {
        /// The source format that was requested.
        from: Format,
        /// The target format that was requested.
        to: Format,
    },

    /// The input bytes could not be parsed as `format`.
    ///
    /// Return this (or [`ConvertError::UnsupportedFeature`]) for *every* malformed-input case a
    /// converter can detect. Untrusted bytes reaching a `panic!`/`unwrap`/`expect` instead of
    /// this variant is treated as a security bug in this crate — see `docs/SECURITY.md`.
    MalformedInput {
        /// The format the input was supposed to be, i.e. the `from` the converter was invoked
        /// with.
        format: Format,
    },

    /// The input parsed correctly but uses a feature of `format` this converter does not (yet)
    /// support — e.g. progressive JPEG when only baseline is implemented.
    UnsupportedFeature {
        /// The format whose parser rejected the input.
        format: Format,
        /// A short, stable, machine-readable identifier (`"progressive-scan"`, not "This JPEG
        /// uses progressive encoding, which we don't support yet") — the UI localizes it, same
        /// as every other identifier in this enum.
        feature: &'static str,
    },

    /// The input exceeds the limit set in [`crate::ConvertOptions::max_input_bytes`]. Returned by
    /// [`crate::convert`]/[`crate::convert_with`] before a converter ever sees the bytes.
    SizeLimitExceeded {
        /// The configured limit, in bytes.
        limit: u64,
        /// The actual input size, in bytes.
        actual: u64,
    },

    /// A [`crate::ProgressSink::is_cancelled`] check returned `true` and the conversion stopped
    /// early. Not a failure of the conversion itself — the caller asked for this.
    Cancelled,

    /// Something went wrong that is not the caller's fault and not a malformed-input case — an
    /// internal invariant broke.
    Internal {
        /// A short static code for logs/bug reports (e.g. `"registry-lock-poisoned"`) — not a
        /// user-facing message.
        detail: &'static str,
    },
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::UnsupportedPair { from, to } => {
                write!(
                    f,
                    "unsupported conversion pair: {} -> {}",
                    from.id(),
                    to.id()
                )
            }
            ConvertError::MalformedInput { format } => {
                write!(f, "malformed input for format: {}", format.id())
            }
            ConvertError::UnsupportedFeature { format, feature } => {
                write!(
                    f,
                    "unsupported feature '{feature}' for format: {}",
                    format.id()
                )
            }
            ConvertError::SizeLimitExceeded { limit, actual } => {
                write!(
                    f,
                    "input size {actual} bytes exceeds limit of {limit} bytes"
                )
            }
            ConvertError::Cancelled => write!(f, "conversion cancelled"),
            ConvertError::Internal { detail } => write!(f, "internal error: {detail}"),
        }
    }
}

impl std::error::Error for ConvertError {}
