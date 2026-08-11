//! A trivial passthrough [`Converter`], registered for `Format::PlainText -> Format::PlainText`.
//!
//! This exists purely so the foundation (trait + registry + dispatch + error paths) has
//! something end-to-end to compile and test against before the first real converter (units,
//! image, text, CAD) lands. Once real converters make it unnecessary to keep the default
//! registry non-empty for tests, this module's *registration* should be dropped — the type
//! itself is a reasonable template for a "does nothing but must still respect the contract" test
//! double later.

use crate::{ConvertError, ConvertOptions, Converter, Format};

/// Returns `input` unchanged.
///
/// Still respects the full [`Converter`] contract: it checks cancellation and reports progress
/// like a real converter would, so it is a faithful (if computationally trivial) example of the
/// hooks every converter is expected to use.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityConverter;

impl Converter for IdentityConverter {
    fn convert(
        &self,
        input: &[u8],
        _from: Format,
        _to: Format,
        options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        options.report_progress(1.0);
        Ok(input.to_vec())
    }
}
