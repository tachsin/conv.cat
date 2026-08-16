//! The [`Format`] catalog and the `(from, to) -> Converter` dispatch table.

use std::collections::HashMap;

use crate::converter::Converter;

/// The broad grouping a [`Format`] belongs to, matching the categories conv.cat's UI groups
/// formats into (see the root README's format table).
///
/// Marked `#[non_exhaustive]` for the same reason [`Format`] is: this list is expected to be
/// complete for a while, but treat it as closed-for-matching, not closed-for-growth.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Raster/vector image formats (PNG, JPEG, WebP, BMP, …).
    Image,
    /// Structured or plain text formats (CSV, JSON, HTML, Markdown, plain text, …).
    Text,
    /// Unit-of-measurement conversions (length, mass, temperature, …). Not a file format, but
    /// modeled the same way so the registry and dispatch logic don't need a special case for it.
    Units,
}

/// A file or data format `conv-core` knows about.
///
/// Adding a new format means adding a variant here plus its metadata below, then registering a
/// [`Converter`] for it — see `docs/adding-a-format.md` for the full walkthrough. The variant
/// name, [`Format::id`], the `packages/data` catalog entry id, and the i18n key segment must all
/// match; nothing currently enforces that automatically.
///
/// Marked `#[non_exhaustive]`: every format-by-format backlog ticket after this one adds a
/// variant here, and without this attribute each one would be a breaking change for every
/// downstream crate that matches on `Format` exhaustively (`crates/conv-wasm`, a future CLI,
/// third-party embedders of this MIT-licensed crate). Match with a wildcard arm, or use
/// `matches!`. This does not stop you from *constructing* an existing variant like
/// `Format::PlainText` from another crate — only from matching on the enum without a fallback arm.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// Placeholder text format used only to exercise the [`Converter`]/[`Registry`] pipeline end
    /// to end before any real format lands — see [`crate::formats::identity`]. Not part of the
    /// target format catalog in the root README; do not build real functionality on top of it.
    PlainText,

    // ─── Image — first real, file-shaped category ──────────────────────────────────────────
    //
    // See `crate::formats::image` module docs for why BMP/QOI specifically (both cheap to
    // hand-roll, deterministic/lossless, no new dependency) rather than the full target catalog
    // (PNG/JPEG/WebP/...) in one shot.
    /// Uncompressed, 24-bit Windows BMP.
    Bmp,
    /// [QOI](https://qoiformat.org/) ("Quite OK Image") — a small, deterministic lossless format.
    Qoi,

    // ─── Units — one variant per category, not per unit ────────────────────────────────────
    //
    // See `crate::formats::units` module docs for why: units aren't file-shaped, so the actual
    // from-unit/to-unit identity travels in the payload bytes, not in `Format`. Each is
    // registered as a self-pair (e.g. `UnitsLength -> UnitsLength`) in `default_registry` below.
    // `Format::id()` follows `units_<category_id>` mechanically — keep that convention if a
    // future category is added here.
    /// Length (meter, foot, light-year, …).
    UnitsLength,
    /// Mass (kilogram, pound, …).
    UnitsMass,
    /// Volume (liter, gallon, …).
    UnitsVolume,
    /// Cooking measurements (cup, tablespoon, pinch, …) — genuinely linear, kept as its own
    /// category (not folded into `UnitsVolume`) to match the legacy catalog and the root
    /// README's "genuinely niche stuff" pitch.
    UnitsCooking,
    /// Temperature (Celsius, Fahrenheit, Kelvin, …) — affine, not linear-through-origin; see
    /// [`crate::formats::units::catalog::UnitConversion::Affine`].
    UnitsTemperature,
    /// Fuel consumption (L/100km, mpg, km/L, …) — mixes linear and reciprocal units; see
    /// [`crate::formats::units::catalog::UnitConversion::Reciprocal`].
    UnitsFuelConsumption,
    /// Life age — species years ⇄ human years, including cat/dog years. Piecewise/lifespan
    /// algorithm, not a factor table; see [`crate::formats::units::life_age`].
    UnitsLifeAge,
    /// Clothing size — chart-based lookup with text-label values as well as numbers; see
    /// [`crate::formats::units::clothing_size`].
    UnitsClothingSize,
}

impl Format {
    /// Stable, lowercase identifier. This is the join key with the `packages/data` catalog and
    /// i18n key segments — changing it is a breaking change across both.
    pub fn id(&self) -> &'static str {
        match self {
            Format::PlainText => "plain_text",
            Format::Bmp => "bmp",
            Format::Qoi => "qoi",
            Format::UnitsLength => "units_length",
            Format::UnitsMass => "units_mass",
            Format::UnitsVolume => "units_volume",
            Format::UnitsCooking => "units_cooking",
            Format::UnitsTemperature => "units_temperature",
            Format::UnitsFuelConsumption => "units_fuel_consumption",
            Format::UnitsLifeAge => "units_life_age",
            Format::UnitsClothingSize => "units_clothing_size",
        }
    }

    /// The [`Category`] this format belongs to.
    pub fn category(&self) -> Category {
        match self {
            Format::PlainText => Category::Text,
            Format::Bmp | Format::Qoi => Category::Image,
            Format::UnitsLength
            | Format::UnitsMass
            | Format::UnitsVolume
            | Format::UnitsCooking
            | Format::UnitsTemperature
            | Format::UnitsFuelConsumption
            | Format::UnitsLifeAge
            | Format::UnitsClothingSize => Category::Units,
        }
    }

    /// The MIME type used when this format is read from or written to a browser `Blob`/file
    /// input, or exposed via HTTP.
    ///
    /// Units formats aren't real files — there is no browser `Blob` or HTTP body with this MIME
    /// type in practice — but every [`Format`] carries one by convention (see this crate's
    /// module docs on units being "not a file format, but modeled the same way"), so this is a
    /// single conv.cat-internal placeholder rather than something meaningful to a real HTTP
    /// client.
    pub fn mime(&self) -> &'static str {
        match self {
            Format::PlainText => "text/plain",
            Format::Bmp => "image/bmp",
            Format::Qoi => "image/qoi",
            Format::UnitsLength
            | Format::UnitsMass
            | Format::UnitsVolume
            | Format::UnitsCooking
            | Format::UnitsTemperature
            | Format::UnitsFuelConsumption
            | Format::UnitsLifeAge
            | Format::UnitsClothingSize => "application/x-conv-unit",
        }
    }

    /// File extensions (without the leading dot) commonly used for this format, most preferred
    /// first. Empty for every units format — they have no file extension.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Format::PlainText => &["txt"],
            Format::Bmp => &["bmp"],
            Format::Qoi => &["qoi"],
            Format::UnitsLength
            | Format::UnitsMass
            | Format::UnitsVolume
            | Format::UnitsCooking
            | Format::UnitsTemperature
            | Format::UnitsFuelConsumption
            | Format::UnitsLifeAge
            | Format::UnitsClothingSize => &[],
        }
    }

    /// Whether any registered converter can decode this format, i.e. use it as a `from`.
    ///
    /// This describes the format itself, independent of which specific pairs are registered —
    /// e.g. HEIC is decode-only in conv.cat's target catalog, so `can_write` would be `false`
    /// for it even once a `Heic -> Png` converter exists. Every units format is both readable and
    /// writable at the `Format` level even though individual *units* within a category can be
    /// [`crate::formats::units::catalog::UnitConversion::Unconvertible`] — that finer-grained
    /// support is enforced per-request by `UnitsConverter`, not at this level.
    pub fn can_read(&self) -> bool {
        match self {
            Format::PlainText => true,
            Format::Bmp | Format::Qoi => true,
            Format::UnitsLength
            | Format::UnitsMass
            | Format::UnitsVolume
            | Format::UnitsCooking
            | Format::UnitsTemperature
            | Format::UnitsFuelConsumption
            | Format::UnitsLifeAge
            | Format::UnitsClothingSize => true,
        }
    }

    /// Whether any registered converter can encode this format, i.e. use it as a `to`.
    pub fn can_write(&self) -> bool {
        match self {
            Format::PlainText => true,
            Format::Bmp | Format::Qoi => true,
            Format::UnitsLength
            | Format::UnitsMass
            | Format::UnitsVolume
            | Format::UnitsCooking
            | Format::UnitsTemperature
            | Format::UnitsFuelConsumption
            | Format::UnitsLifeAge
            | Format::UnitsClothingSize => true,
        }
    }
}

/// The `(from, to) -> Converter` dispatch table.
///
/// Construct one with [`Registry::new`], populate it with [`Registry::register`], and look
/// converters up with [`Registry::resolve`]. Most callers never build their own — [`crate::convert`]
/// dispatches through a process-wide default registry — but the type is public so a caller that
/// wants a custom or reduced format set can build one and drive it directly with
/// [`crate::convert_with`].
#[derive(Default)]
pub struct Registry {
    converters: HashMap<(Format, Format), Box<dyn Converter>>,
}

impl Registry {
    /// Creates an empty registry with no converters registered.
    pub fn new() -> Self {
        Registry {
            converters: HashMap::new(),
        }
    }

    /// Registers `converter` as the handler for `from -> to`.
    ///
    /// Registering the same pair twice replaces the previous entry rather than erroring — this
    /// is deliberate, so a test or an embedder can swap in a fake without extra API surface.
    pub fn register(&mut self, from: Format, to: Format, converter: Box<dyn Converter>) {
        self.converters.insert((from, to), converter);
    }

    /// Looks up the converter registered for `from -> to`, if any.
    pub fn resolve(&self, from: Format, to: Format) -> Option<&dyn Converter> {
        self.converters.get(&(from, to)).map(Box::as_ref)
    }
}
