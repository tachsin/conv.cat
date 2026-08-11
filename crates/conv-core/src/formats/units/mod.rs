//! Unit-of-measurement conversions — the first "vertical slice" ported into `conv-core`, and the
//! reference implementation for any future category that isn't file-shaped (see
//! `docs/adding-a-format.md`'s callout after the QOI walkthrough for a pointer here).
//!
//! Covers a representative subset of the legacy catalog: `length`, `mass`, `volume`, `cooking`,
//! `temperature`, `fuel_consumption`, `life_age`, `clothing_size`. See
//! `packages/data/src/units/README.md` for why these eight (not the legacy catalog's full 49
//! categories) and the exact list of units within them that have no conversion data — the legacy
//! app shipped catalog metadata for a handful of units it never actually computed either
//! (temperature's `gas_mark`, `clothing_size`'s kids/bra sizing, …); this port is honest about
//! that rather than fabricating results for them.
//!
//! ## Why this doesn't look like a normal `Converter`
//!
//! [`crate::Converter`] is `&[u8] -> Vec<u8>`, built for file formats. A unit conversion is
//! `(value, from_unit, to_unit) -> value`, not bytes-to-bytes — [`crate::Category::Units`] already
//! existed with a doc comment anticipating this ("not a file format, but modeled the same way"),
//! but no payload convention existed until this module. The convention:
//!
//! - **One [`crate::Format`] variant per unit *category***, not per unit — `Format::UnitsLength`,
//!   `Format::UnitsTemperature`, etc. The legacy catalog's 977 units across 49 categories would
//!   mean 977 hand-maintained `Format` variants, which doesn't scale; a category is a manageable,
//!   hand-writable number (see `crates/conv-core/src/registry.rs`). Each is registered as a
//!   self-pair (`UnitsLength -> UnitsLength`), exactly like `Format::PlainText -> Format::PlainText`
//!   — see [`crate::formats::identity`].
//! - **The actual from-unit/to-unit identity travels in the payload, not the `Format`.** See
//!   [`payload`] for the exact wire protocol (`"<from_unit_id>|<to_unit_id>|<value>"` as UTF-8
//!   text — `conv-core` has zero runtime dependencies, so this is hand-parsed, not JSON).
//!
//! ## The five conversion models
//!
//! - [`catalog::UnitConversion::Linear`] — the overwhelming majority of units: `x * factor`.
//! - [`catalog::UnitConversion::Affine`] — `x * scale + offset`; only `temperature` needs this
//!   (Celsius/Fahrenheit/etc. all have a nonzero offset from Kelvin — this is the "offset, not
//!   just scale" case the units vertical-slice ticket specifically asked to get right, and which
//!   the legacy app never actually implemented despite shipping the catalog data for it).
//! - [`catalog::UnitConversion::Reciprocal`] — `k / x`, a self-inverse transform; only
//!   `fuel_consumption`'s km-per-liter-style units need this.
//! - [`life_age`] — piecewise-interpolated curves / a lifespan ratio, a hand-ported algorithm
//!   (species years ⇄ human years), not table data.
//! - [`clothing_size`] — chart-based lookup with **text-label** values as well as numbers, also a
//!   hand-ported algorithm. The only category where the wire payload's value field isn't always a
//!   plain number.
//!
//! [`catalog::UnitConversion::Unconvertible`] is not a sixth model — it marks a unit that has no
//! conversion data in the legacy catalog *either*. Naming one in a request returns
//! [`crate::ConvertError::UnsupportedFeature`], never a fabricated result.
//!
//! ## Precision
//!
//! Every conversion runs in `f64` throughout, with no intermediate rounding beyond whatever
//! IEEE 754 itself introduces at each arithmetic step — [`payload::format_number`] only rounds
//! for *display* (Rust's shortest-round-trippable-decimal `f64::to_string()`), never
//! mid-calculation. A round trip through the same two units (`A -> B -> A`) is exact to within a
//! handful of ULPs for the linear/affine/reciprocal models. `crates/conv-core/tests/golden.rs`'s
//! `unit_round_trips_stay_within_tolerance` property test exercises every unit in every
//! table-driven (generic-model) category with a self round trip (`unit -> SI -> unit`) and
//! asserts the result is within `1e-9` relative error of the original value — chosen as
//! comfortably above `f64`'s own precision floor (~1e-15 relative) for the value ranges this
//! catalog's factors actually produce, while still tight enough to catch a real bug (a wrong
//! factor, a sign error) rather than just tolerating rounding noise. That test is the executable
//! version of this paragraph — read it if this tolerance ever needs to change. A chained
//! multi-hop UI conversion (unit A -> SI -> unit B -> SI -> unit C, as a real converter UI would
//! do across two dropdown changes) composes this same per-hop error additively, not
//! multiplicatively — each hop is one independent `f64` op via [`catalog::convert_via_si`]'s
//! `to_si`/`from_si` pair, not a recursive amplification.

pub mod catalog;
pub mod clothing_size;
mod converter;
pub mod generated;
pub mod life_age;
pub mod payload;

pub use converter::UnitsConverter;
