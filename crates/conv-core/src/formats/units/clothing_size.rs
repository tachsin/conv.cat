//! Clothing size conversions via body-measurement lookup tables — the most complex category in
//! this ticket's scope, and genuinely a differentiator (see the root README's "genuinely niche
//! stuff" pitch). Hand-ported, not table-driven, from the more complete of the two legacy
//! implementations: `conv.cat_legacy/conv-next/lib/clothing-size.ts` (the legacy Rust version
//! never wired this in at all — see `packages/data/src/units/README.md`).
//!
//! Unlike every other category in scope, values here can be **numbers or text labels**
//! (`"M"`, `"XL"`, `"42"`) in both directions — see [`ClothingValue`] and
//! [`super::payload`]'s protocol docs.
//!
//! Ten units have no chart data in the legacy source (kids' sizing, bra sizing) and are honest
//! gaps here too — see `packages/data/src/units/README.md` for the exact list.

use crate::{ConvertError, Format};

use super::payload::format_number;

/// A parsed clothing-size value: either a measurement/size number, or a normalized size label.
#[derive(Debug, Clone, PartialEq)]
pub enum ClothingValue {
    /// A number — a chart index (`8`, `42`) or a raw measurement (`92` cm).
    Number(f64),
    /// A normalized size label (`"XL"`, `"XXS"`).
    Label(String),
}

// ─── Charts (faithfully transcribed from clothing-size.ts's WOMENS_TOPS/MENS_TOPS/HAT_SIZES/GLOVE_SIZES) ───

struct WomensRow {
    bust: f64,
    waist: f64,
    hip: f64,
    int_num: f64,
    letter: &'static str,
    letter_us: &'static str,
    us: f64,
    uk: f64,
    eu: f64,
    it: f64,
    fr: f64,
    jp: f64,
    cn: f64,
    au: f64,
    kr: f64,
    ru: f64,
    br: f64,
    mx: f64,
    dress_us: f64,
    dress_uk: f64,
    dress_eu: f64,
    dress_fr: f64,
}

struct MensRow {
    chest: f64,
    /// Present in the legacy `MensRow` type but never actually read by any convert path — see
    /// `resolve_mens_row`'s doc comment for why. Kept for fidelity with the legacy struct shape.
    #[allow(dead_code)]
    chest_in: f64,
    int_num: f64,
    letter: &'static str,
    letter_us: &'static str,
    us: f64,
    uk: f64,
    eu: f64,
    it: f64,
    fr: f64,
    jp: f64,
    cn: f64,
    au: f64,
    kr: f64,
    ru: f64,
    br: f64,
    mx: f64,
}

struct HatRow {
    head: f64,
    us: f64,
    uk: f64,
    eu: f64,
    fr: f64,
}

struct GloveRow {
    /// Present in the legacy `GloveRow` type but never read by any convert path — unlike
    /// `HatRow.head` (which `neck_cm` maps to specially), no glove unit id maps to this column in
    /// the legacy source. Kept for fidelity with the legacy struct shape rather than dropped.
    #[allow(dead_code)]
    hand: f64,
    us: f64,
    uk: f64,
    eu: f64,
    jp: f64,
}

/// Approximate women's tops / dress chart (bust & waist in cm).
const WOMENS_TOPS: &[WomensRow] = &[
    WomensRow {
        bust: 76.0,
        waist: 58.0,
        hip: 84.0,
        int_num: 30.0,
        letter: "XXS",
        letter_us: "XXS",
        us: 0.0,
        uk: 4.0,
        eu: 30.0,
        it: 38.0,
        fr: 32.0,
        jp: 5.0,
        cn: 155.0,
        au: 4.0,
        kr: 44.0,
        ru: 38.0,
        br: 34.0,
        mx: 0.0,
        dress_us: 0.0,
        dress_uk: 4.0,
        dress_eu: 30.0,
        dress_fr: 32.0,
    },
    WomensRow {
        bust: 80.0,
        waist: 62.0,
        hip: 88.0,
        int_num: 32.0,
        letter: "XS",
        letter_us: "XS",
        us: 2.0,
        uk: 6.0,
        eu: 32.0,
        it: 40.0,
        fr: 34.0,
        jp: 7.0,
        cn: 160.0,
        au: 6.0,
        kr: 44.0,
        ru: 40.0,
        br: 36.0,
        mx: 2.0,
        dress_us: 2.0,
        dress_uk: 6.0,
        dress_eu: 32.0,
        dress_fr: 34.0,
    },
    WomensRow {
        bust: 84.0,
        waist: 66.0,
        hip: 92.0,
        int_num: 34.0,
        letter: "XS",
        letter_us: "S",
        us: 4.0,
        uk: 8.0,
        eu: 34.0,
        it: 42.0,
        fr: 36.0,
        jp: 9.0,
        cn: 165.0,
        au: 8.0,
        kr: 55.0,
        ru: 42.0,
        br: 38.0,
        mx: 4.0,
        dress_us: 4.0,
        dress_uk: 8.0,
        dress_eu: 34.0,
        dress_fr: 36.0,
    },
    WomensRow {
        bust: 88.0,
        waist: 70.0,
        hip: 96.0,
        int_num: 36.0,
        letter: "S",
        letter_us: "S",
        us: 6.0,
        uk: 10.0,
        eu: 36.0,
        it: 44.0,
        fr: 38.0,
        jp: 11.0,
        cn: 170.0,
        au: 10.0,
        kr: 55.0,
        ru: 44.0,
        br: 40.0,
        mx: 6.0,
        dress_us: 6.0,
        dress_uk: 10.0,
        dress_eu: 36.0,
        dress_fr: 38.0,
    },
    WomensRow {
        bust: 92.0,
        waist: 74.0,
        hip: 100.0,
        int_num: 38.0,
        letter: "S",
        letter_us: "M",
        us: 8.0,
        uk: 12.0,
        eu: 38.0,
        it: 46.0,
        fr: 40.0,
        jp: 13.0,
        cn: 175.0,
        au: 12.0,
        kr: 66.0,
        ru: 46.0,
        br: 42.0,
        mx: 8.0,
        dress_us: 8.0,
        dress_uk: 12.0,
        dress_eu: 38.0,
        dress_fr: 40.0,
    },
    WomensRow {
        bust: 96.0,
        waist: 78.0,
        hip: 104.0,
        int_num: 40.0,
        letter: "M",
        letter_us: "M",
        us: 10.0,
        uk: 14.0,
        eu: 40.0,
        it: 48.0,
        fr: 42.0,
        jp: 15.0,
        cn: 180.0,
        au: 14.0,
        kr: 66.0,
        ru: 48.0,
        br: 44.0,
        mx: 10.0,
        dress_us: 10.0,
        dress_uk: 14.0,
        dress_eu: 40.0,
        dress_fr: 42.0,
    },
    WomensRow {
        bust: 100.0,
        waist: 82.0,
        hip: 108.0,
        int_num: 42.0,
        letter: "M",
        letter_us: "L",
        us: 12.0,
        uk: 16.0,
        eu: 42.0,
        it: 50.0,
        fr: 44.0,
        jp: 17.0,
        cn: 185.0,
        au: 16.0,
        kr: 77.0,
        ru: 50.0,
        br: 46.0,
        mx: 12.0,
        dress_us: 12.0,
        dress_uk: 16.0,
        dress_eu: 42.0,
        dress_fr: 44.0,
    },
    WomensRow {
        bust: 104.0,
        waist: 86.0,
        hip: 112.0,
        int_num: 44.0,
        letter: "L",
        letter_us: "L",
        us: 14.0,
        uk: 18.0,
        eu: 44.0,
        it: 52.0,
        fr: 46.0,
        jp: 19.0,
        cn: 190.0,
        au: 18.0,
        kr: 77.0,
        ru: 52.0,
        br: 48.0,
        mx: 14.0,
        dress_us: 14.0,
        dress_uk: 18.0,
        dress_eu: 44.0,
        dress_fr: 46.0,
    },
    WomensRow {
        bust: 108.0,
        waist: 90.0,
        hip: 116.0,
        int_num: 46.0,
        letter: "L",
        letter_us: "XL",
        us: 16.0,
        uk: 20.0,
        eu: 46.0,
        it: 54.0,
        fr: 48.0,
        jp: 21.0,
        cn: 195.0,
        au: 20.0,
        kr: 88.0,
        ru: 54.0,
        br: 50.0,
        mx: 16.0,
        dress_us: 16.0,
        dress_uk: 20.0,
        dress_eu: 46.0,
        dress_fr: 48.0,
    },
    WomensRow {
        bust: 112.0,
        waist: 94.0,
        hip: 120.0,
        int_num: 48.0,
        letter: "XL",
        letter_us: "XL",
        us: 18.0,
        uk: 22.0,
        eu: 48.0,
        it: 56.0,
        fr: 50.0,
        jp: 23.0,
        cn: 200.0,
        au: 22.0,
        kr: 88.0,
        ru: 56.0,
        br: 52.0,
        mx: 18.0,
        dress_us: 18.0,
        dress_uk: 22.0,
        dress_eu: 48.0,
        dress_fr: 50.0,
    },
    WomensRow {
        bust: 116.0,
        waist: 98.0,
        hip: 124.0,
        int_num: 50.0,
        letter: "XL",
        letter_us: "XXL",
        us: 20.0,
        uk: 24.0,
        eu: 50.0,
        it: 58.0,
        fr: 52.0,
        jp: 25.0,
        cn: 205.0,
        au: 24.0,
        kr: 99.0,
        ru: 58.0,
        br: 54.0,
        mx: 20.0,
        dress_us: 20.0,
        dress_uk: 24.0,
        dress_eu: 50.0,
        dress_fr: 52.0,
    },
    WomensRow {
        bust: 120.0,
        waist: 102.0,
        hip: 128.0,
        int_num: 52.0,
        letter: "XXL",
        letter_us: "XXL",
        us: 22.0,
        uk: 26.0,
        eu: 52.0,
        it: 60.0,
        fr: 54.0,
        jp: 27.0,
        cn: 210.0,
        au: 26.0,
        kr: 99.0,
        ru: 60.0,
        br: 56.0,
        mx: 22.0,
        dress_us: 22.0,
        dress_uk: 26.0,
        dress_eu: 52.0,
        dress_fr: 54.0,
    },
];

const MENS_TOPS: &[MensRow] = &[
    MensRow {
        chest: 84.0,
        chest_in: 33.0,
        int_num: 42.0,
        letter: "XS",
        letter_us: "XS",
        us: 34.0,
        uk: 34.0,
        eu: 44.0,
        it: 44.0,
        fr: 44.0,
        jp: 87.0,
        cn: 165.0,
        au: 87.0,
        kr: 90.0,
        ru: 44.0,
        br: 44.0,
        mx: 34.0,
    },
    MensRow {
        chest: 88.0,
        chest_in: 35.0,
        int_num: 44.0,
        letter: "S",
        letter_us: "S",
        us: 36.0,
        uk: 36.0,
        eu: 46.0,
        it: 46.0,
        fr: 46.0,
        jp: 90.0,
        cn: 170.0,
        au: 90.0,
        kr: 95.0,
        ru: 46.0,
        br: 46.0,
        mx: 36.0,
    },
    MensRow {
        chest: 92.0,
        chest_in: 36.0,
        int_num: 46.0,
        letter: "S",
        letter_us: "M",
        us: 38.0,
        uk: 38.0,
        eu: 48.0,
        it: 48.0,
        fr: 48.0,
        jp: 93.0,
        cn: 175.0,
        au: 93.0,
        kr: 100.0,
        ru: 48.0,
        br: 48.0,
        mx: 38.0,
    },
    MensRow {
        chest: 96.0,
        chest_in: 38.0,
        int_num: 48.0,
        letter: "M",
        letter_us: "M",
        us: 40.0,
        uk: 40.0,
        eu: 50.0,
        it: 50.0,
        fr: 50.0,
        jp: 96.0,
        cn: 180.0,
        au: 96.0,
        kr: 105.0,
        ru: 50.0,
        br: 50.0,
        mx: 40.0,
    },
    MensRow {
        chest: 100.0,
        chest_in: 39.0,
        int_num: 50.0,
        letter: "M",
        letter_us: "L",
        us: 42.0,
        uk: 42.0,
        eu: 52.0,
        it: 52.0,
        fr: 52.0,
        jp: 99.0,
        cn: 185.0,
        au: 99.0,
        kr: 110.0,
        ru: 52.0,
        br: 52.0,
        mx: 42.0,
    },
    MensRow {
        chest: 104.0,
        chest_in: 41.0,
        int_num: 52.0,
        letter: "L",
        letter_us: "L",
        us: 44.0,
        uk: 44.0,
        eu: 54.0,
        it: 54.0,
        fr: 54.0,
        jp: 102.0,
        cn: 190.0,
        au: 102.0,
        kr: 115.0,
        ru: 54.0,
        br: 54.0,
        mx: 44.0,
    },
    MensRow {
        chest: 108.0,
        chest_in: 42.0,
        int_num: 54.0,
        letter: "L",
        letter_us: "XL",
        us: 46.0,
        uk: 46.0,
        eu: 56.0,
        it: 56.0,
        fr: 56.0,
        jp: 105.0,
        cn: 195.0,
        au: 105.0,
        kr: 120.0,
        ru: 56.0,
        br: 56.0,
        mx: 46.0,
    },
    MensRow {
        chest: 112.0,
        chest_in: 44.0,
        int_num: 56.0,
        letter: "XL",
        letter_us: "XL",
        us: 48.0,
        uk: 48.0,
        eu: 58.0,
        it: 58.0,
        fr: 58.0,
        jp: 108.0,
        cn: 200.0,
        au: 108.0,
        kr: 125.0,
        ru: 58.0,
        br: 58.0,
        mx: 48.0,
    },
    MensRow {
        chest: 116.0,
        chest_in: 46.0,
        int_num: 58.0,
        letter: "XXL",
        letter_us: "XXL",
        us: 50.0,
        uk: 50.0,
        eu: 60.0,
        it: 60.0,
        fr: 60.0,
        jp: 111.0,
        cn: 205.0,
        au: 111.0,
        kr: 130.0,
        ru: 60.0,
        br: 60.0,
        mx: 50.0,
    },
];

const HAT_SIZES: &[HatRow] = &[
    HatRow {
        head: 52.0,
        us: 6.5,
        uk: 6.5,
        eu: 52.0,
        fr: 52.0,
    },
    HatRow {
        head: 53.0,
        us: 6.625,
        uk: 6.625,
        eu: 53.0,
        fr: 53.0,
    },
    HatRow {
        head: 54.0,
        us: 6.75,
        uk: 6.75,
        eu: 54.0,
        fr: 54.0,
    },
    HatRow {
        head: 55.0,
        us: 6.875,
        uk: 6.875,
        eu: 55.0,
        fr: 55.0,
    },
    HatRow {
        head: 56.0,
        us: 7.0,
        uk: 7.0,
        eu: 56.0,
        fr: 56.0,
    },
    HatRow {
        head: 57.0,
        us: 7.125,
        uk: 7.125,
        eu: 57.0,
        fr: 57.0,
    },
    HatRow {
        head: 58.0,
        us: 7.25,
        uk: 7.25,
        eu: 58.0,
        fr: 58.0,
    },
    HatRow {
        head: 59.0,
        us: 7.375,
        uk: 7.375,
        eu: 59.0,
        fr: 59.0,
    },
    HatRow {
        head: 60.0,
        us: 7.5,
        uk: 7.5,
        eu: 60.0,
        fr: 60.0,
    },
    HatRow {
        head: 61.0,
        us: 7.625,
        uk: 7.625,
        eu: 61.0,
        fr: 61.0,
    },
    HatRow {
        head: 62.0,
        us: 7.75,
        uk: 7.75,
        eu: 62.0,
        fr: 62.0,
    },
];

const GLOVE_SIZES: &[GloveRow] = &[
    GloveRow {
        hand: 17.0,
        us: 6.0,
        uk: 6.0,
        eu: 6.0,
        jp: 17.0,
    },
    GloveRow {
        hand: 18.0,
        us: 6.5,
        uk: 6.5,
        eu: 7.0,
        jp: 18.0,
    },
    GloveRow {
        hand: 19.0,
        us: 7.0,
        uk: 7.0,
        eu: 8.0,
        jp: 19.0,
    },
    GloveRow {
        hand: 20.0,
        us: 7.5,
        uk: 7.5,
        eu: 9.0,
        jp: 20.0,
    },
    GloveRow {
        hand: 21.0,
        us: 8.0,
        uk: 8.0,
        eu: 10.0,
        jp: 21.0,
    },
    GloveRow {
        hand: 22.0,
        us: 8.5,
        uk: 8.5,
        eu: 11.0,
        jp: 22.0,
    },
    GloveRow {
        hand: 23.0,
        us: 9.0,
        uk: 9.0,
        eu: 12.0,
        jp: 23.0,
    },
    GloveRow {
        hand: 24.0,
        us: 10.0,
        uk: 10.0,
        eu: 13.0,
        jp: 24.0,
    },
];

// ─── Column maps (unit id -> chart column) ─────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum WomensColumn {
    IntNum,
    Letter,
    LetterUs,
    Us,
    Uk,
    Eu,
    It,
    Fr,
    Jp,
    Cn,
    Au,
    Kr,
    Ru,
    Br,
    Mx,
    DressUs,
    DressUk,
    DressEu,
    DressFr,
}

const WOMENS_UNITS: &[(&str, WomensColumn)] = &[
    ("international_number", WomensColumn::IntNum),
    ("international_letter", WomensColumn::Letter),
    ("us_womens_numeric", WomensColumn::Us),
    ("us_womens_letter", WomensColumn::LetterUs),
    ("uk_womens", WomensColumn::Uk),
    ("eu_womens", WomensColumn::Eu),
    ("it_womens", WomensColumn::It),
    ("fr_womens", WomensColumn::Fr),
    ("jp_womens", WomensColumn::Jp),
    ("cn_womens", WomensColumn::Cn),
    ("au_womens", WomensColumn::Au),
    ("kr_womens", WomensColumn::Kr),
    ("ru_womens", WomensColumn::Ru),
    ("br_womens", WomensColumn::Br),
    ("mx_womens", WomensColumn::Mx),
    ("dress_us", WomensColumn::DressUs),
    ("dress_uk", WomensColumn::DressUk),
    ("dress_eu", WomensColumn::DressEu),
    ("dress_fr", WomensColumn::DressFr),
];

#[derive(Clone, Copy, PartialEq)]
enum MensColumn {
    IntNum,
    Letter,
    LetterUs,
    Us,
    Uk,
    Eu,
    It,
    Fr,
    Jp,
    Cn,
    Au,
    Kr,
    Ru,
    Br,
    Mx,
}

// NOTE: `international_number`/`international_letter` also appear here, but `chart_kind` always
// classifies those two ids as "womens" (see below) before this map is ever consulted for them —
// a legacy quirk (the two tables use different numeric scales for the same unit id) preserved
// faithfully rather than silently "fixed". See packages/data/src/units/README.md.
const MENS_UNITS: &[(&str, MensColumn)] = &[
    ("international_number", MensColumn::IntNum),
    ("international_letter", MensColumn::Letter),
    ("us_mens_numeric", MensColumn::Us),
    ("us_mens_letter", MensColumn::LetterUs),
    ("uk_mens", MensColumn::Uk),
    ("eu_mens", MensColumn::Eu),
    ("it_mens", MensColumn::It),
    ("fr_mens", MensColumn::Fr),
    ("jp_mens", MensColumn::Jp),
    ("cn_mens", MensColumn::Cn),
    ("au_mens", MensColumn::Au),
    ("kr_mens", MensColumn::Kr),
    ("ru_mens", MensColumn::Ru),
    ("br_mens", MensColumn::Br),
    ("mx_mens", MensColumn::Mx),
];

#[derive(Clone, Copy, PartialEq)]
enum HatColumn {
    Us,
    Uk,
    Eu,
    Fr,
}

const HAT_UNITS: &[(&str, HatColumn)] = &[
    ("hat_us", HatColumn::Us),
    ("hat_uk", HatColumn::Uk),
    ("hat_eu", HatColumn::Eu),
    ("hat_fr", HatColumn::Fr),
];

#[derive(Clone, Copy, PartialEq)]
enum GloveColumn {
    Us,
    Uk,
    Eu,
    Jp,
}

const GLOVE_UNITS: &[(&str, GloveColumn)] = &[
    ("glove_us", GloveColumn::Us),
    ("glove_uk", GloveColumn::Uk),
    ("glove_eu", GloveColumn::Eu),
    ("glove_jp", GloveColumn::Jp),
];

const CM_PER_IN: f64 = 2.54;

/// One cell's value, so a single generic lookup/nearest-match helper works for both numeric and
/// letter columns (mirrors `readCell`'s untyped return in the legacy TS).
enum Cell {
    Number(f64),
    Text(&'static str),
}

fn womens_cell(row: &WomensRow, column: WomensColumn) -> Cell {
    use WomensColumn::*;
    match column {
        IntNum => Cell::Number(row.int_num),
        Letter => Cell::Text(row.letter),
        LetterUs => Cell::Text(row.letter_us),
        Us => Cell::Number(row.us),
        Uk => Cell::Number(row.uk),
        Eu => Cell::Number(row.eu),
        It => Cell::Number(row.it),
        Fr => Cell::Number(row.fr),
        Jp => Cell::Number(row.jp),
        Cn => Cell::Number(row.cn),
        Au => Cell::Number(row.au),
        Kr => Cell::Number(row.kr),
        Ru => Cell::Number(row.ru),
        Br => Cell::Number(row.br),
        Mx => Cell::Number(row.mx),
        DressUs => Cell::Number(row.dress_us),
        DressUk => Cell::Number(row.dress_uk),
        DressEu => Cell::Number(row.dress_eu),
        DressFr => Cell::Number(row.dress_fr),
    }
}

fn is_womens_letter_column(column: WomensColumn) -> bool {
    matches!(column, WomensColumn::Letter | WomensColumn::LetterUs)
}

fn mens_cell(row: &MensRow, column: MensColumn) -> Cell {
    use MensColumn::*;
    match column {
        IntNum => Cell::Number(row.int_num),
        Letter => Cell::Text(row.letter),
        LetterUs => Cell::Text(row.letter_us),
        Us => Cell::Number(row.us),
        Uk => Cell::Number(row.uk),
        Eu => Cell::Number(row.eu),
        It => Cell::Number(row.it),
        Fr => Cell::Number(row.fr),
        Jp => Cell::Number(row.jp),
        Cn => Cell::Number(row.cn),
        Au => Cell::Number(row.au),
        Kr => Cell::Number(row.kr),
        Ru => Cell::Number(row.ru),
        Br => Cell::Number(row.br),
        Mx => Cell::Number(row.mx),
    }
}

fn is_mens_letter_column(column: MensColumn) -> bool {
    matches!(column, MensColumn::Letter | MensColumn::LetterUs)
}

fn hat_cell(row: &HatRow, column: HatColumn) -> Cell {
    match column {
        HatColumn::Us => Cell::Number(row.us),
        HatColumn::Uk => Cell::Number(row.uk),
        HatColumn::Eu => Cell::Number(row.eu),
        HatColumn::Fr => Cell::Number(row.fr),
    }
}

fn glove_cell(row: &GloveRow, column: GloveColumn) -> Cell {
    match column {
        GloveColumn::Us => Cell::Number(row.us),
        GloveColumn::Uk => Cell::Number(row.uk),
        GloveColumn::Eu => Cell::Number(row.eu),
        GloveColumn::Jp => Cell::Number(row.jp),
    }
}

enum ChartKind {
    Womens,
    Mens,
    Hat,
    Glove,
    Linear,
}

fn chart_kind(unit_id: &str) -> Option<ChartKind> {
    if WOMENS_UNITS.iter().any(|(id, _)| *id == unit_id)
        || matches!(
            unit_id,
            "bust_cm" | "waist_cm" | "hip_cm" | "chest_in" | "waist_in" | "hip_in"
        )
    {
        return Some(ChartKind::Womens);
    }
    if MENS_UNITS.iter().any(|(id, _)| *id == unit_id) {
        return Some(ChartKind::Mens);
    }
    if HAT_UNITS.iter().any(|(id, _)| *id == unit_id) || unit_id == "neck_cm" {
        return Some(ChartKind::Hat);
    }
    if GLOVE_UNITS.iter().any(|(id, _)| *id == unit_id) {
        return Some(ChartKind::Glove);
    }
    if matches!(unit_id, "inseam_cm" | "inseam_in") {
        return Some(ChartKind::Linear);
    }
    None
}

const LETTER_UNITS: &[&str] = &["international_letter", "us_womens_letter", "us_mens_letter"];

fn is_letter_unit(unit_id: &str) -> bool {
    LETTER_UNITS.contains(&unit_id)
}

const LETTER_ALIASES: &[(&str, &str)] = &[
    ("3XL", "XXL"),
    ("XXXL", "XXL"),
    ("4XL", "XXL"),
    ("5XL", "XXL"),
    ("2XL", "XXL"),
    ("X-LARGE", "XL"),
    ("X-SMALL", "XS"),
    ("2XS", "XXS"),
    ("XX-SMALL", "XXS"),
];

/// Uppercases, strips whitespace, and resolves a handful of common aliases (`"3XL"` -> `"XXL"`) —
/// matches `normalizeSizeLabel` exactly.
fn normalize_size_label(raw: &str) -> String {
    let compact: String = raw
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    LETTER_ALIASES
        .iter()
        .find(|(alias, _)| *alias == compact)
        .map(|(_, canonical)| (*canonical).to_string())
        .unwrap_or(compact)
}

/// Parses raw wire text into a [`ClothingValue`]. A token containing any ASCII letter, or a
/// `from_id` that's inherently letter-based, is treated as a label; otherwise it must parse as a
/// finite number.
///
/// **Deliberate simplification vs. the legacy TS** (`Number.parseFloat`): this requires the whole
/// trimmed token to be a valid `f64`, rather than JS's "parse a leading numeric prefix, ignore
/// trailing garbage" behavior (`"42abc"` -> `42` in JS). No legacy test exercised that leniency,
/// and the wire protocol controls how numbers are encoded on the Rust side, so this is a
/// documented behavior narrowing, not a hidden bug.
fn parse_input(raw: &str, from_id: &str) -> Option<ClothingValue> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let has_letters = trimmed.chars().any(|c| c.is_ascii_alphabetic());
    if is_letter_unit(from_id) || has_letters {
        let label = normalize_size_label(trimmed);
        if !label.is_empty() {
            return Some(ClothingValue::Label(label));
        }
    }

    if let Ok(num) = trimmed.parse::<f64>() {
        if num.is_finite() {
            return Some(ClothingValue::Number(num));
        }
    }

    None
}

fn womens_measurement_key(unit_id: &str) -> fn(&WomensRow) -> f64 {
    match unit_id {
        "waist_cm" | "waist_in" => |r: &WomensRow| r.waist,
        "hip_cm" | "hip_in" => |r: &WomensRow| r.hip,
        _ => |r: &WomensRow| r.bust,
    }
}

fn to_cm(value: f64, unit_id: &str) -> f64 {
    if unit_id.ends_with("_in") || unit_id == "chest_in" {
        value * CM_PER_IN
    } else {
        value
    }
}

fn from_cm(cm: f64, unit_id: &str) -> f64 {
    if unit_id.ends_with("_in") || unit_id == "chest_in" {
        cm / CM_PER_IN
    } else {
        cm
    }
}

fn cell_matches_label(cell: Cell, target_normalized: &str) -> bool {
    match cell {
        Cell::Text(t) => normalize_size_label(t) == target_normalized,
        Cell::Number(n) => normalize_size_label(&format_number(n)) == target_normalized,
    }
}

fn cell_distance(cell: Cell, value: f64) -> f64 {
    match cell {
        Cell::Number(n) => (n - value).abs(),
        Cell::Text(_) => f64::INFINITY,
    }
}

fn find_row_by_label<'a, T>(
    rows: &'a [T],
    cell: impl Fn(&T) -> Cell,
    target: &str,
) -> Option<&'a T> {
    let target_normalized = normalize_size_label(target);
    rows.iter()
        .find(|row| cell_matches_label(cell(row), &target_normalized))
}

fn nearest_row<T>(rows: &[T], cell: impl Fn(&T) -> Cell, value: f64) -> Option<&T> {
    rows.iter().min_by(|a, b| {
        cell_distance(cell(a), value)
            .partial_cmp(&cell_distance(cell(b), value))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn format_output(cell: Cell, unit_id: &str) -> ClothingValue {
    if is_letter_unit(unit_id) {
        return ClothingValue::Label(match cell {
            Cell::Text(t) => t.to_string(),
            Cell::Number(n) => format_number(n),
        });
    }
    match cell {
        Cell::Text(t) => ClothingValue::Label(t.to_string()),
        Cell::Number(n) => ClothingValue::Number(n),
    }
}

fn resolve_womens_row(from_id: &str, input: &ClothingValue) -> Option<&'static WomensRow> {
    if let Some((_, column)) = WOMENS_UNITS.iter().find(|(id, _)| *id == from_id) {
        let column = *column;
        if is_womens_letter_column(column) {
            let ClothingValue::Label(label) = input else {
                return None;
            };
            return find_row_by_label(WOMENS_TOPS, |r| womens_cell(r, column), label);
        }
        let ClothingValue::Number(value) = input else {
            return None;
        };
        return nearest_row(WOMENS_TOPS, |r| womens_cell(r, column), *value);
    }

    let ClothingValue::Number(value) = input else {
        return None;
    };
    let cm = to_cm(*value, from_id);
    if !(50.0..=140.0).contains(&cm) {
        return None;
    }
    let key = womens_measurement_key(from_id);
    nearest_row(WOMENS_TOPS, |r| Cell::Number(key(r)), cm)
}

fn convert_womens(input: &ClothingValue, from_id: &str, to_id: &str) -> Option<ClothingValue> {
    let row = resolve_womens_row(from_id, input)?;

    if let Some((_, column)) = WOMENS_UNITS.iter().find(|(id, _)| *id == to_id) {
        return Some(format_output(womens_cell(row, *column), to_id));
    }

    if matches!(
        to_id,
        "bust_cm" | "waist_cm" | "hip_cm" | "chest_in" | "waist_in" | "hip_in"
    ) {
        let key = womens_measurement_key(to_id);
        return Some(ClothingValue::Number(from_cm(key(row), to_id)));
    }

    None
}

/// `resolve_mens_row`'s `from_id == "chest_in"` branch mirrors the legacy TS's own
/// `resolveMensRow`, but `chart_kind("chest_in")` always returns `Womens` (see above) — the same
/// as the legacy `chartKind` — so this branch, like its TS counterpart, is unreachable through
/// the public dispatch today. Kept for fidelity rather than "fixed", since removing it would be a
/// silent behavior claim this port isn't making either way; only the `"bust_cm"` half of this
/// check is actually reachable (via `convert_clothing_size_parsed`'s womens->mens bridge, which
/// calls this with the literal id `"bust_cm"`).
fn resolve_mens_row(from_id: &str, input: &ClothingValue) -> Option<&'static MensRow> {
    if let Some((_, column)) = MENS_UNITS.iter().find(|(id, _)| *id == from_id) {
        let column = *column;
        if is_mens_letter_column(column) {
            let ClothingValue::Label(label) = input else {
                return None;
            };
            return find_row_by_label(MENS_TOPS, |r| mens_cell(r, column), label);
        }
        let ClothingValue::Number(value) = input else {
            return None;
        };
        return nearest_row(MENS_TOPS, |r| mens_cell(r, column), *value);
    }

    let ClothingValue::Number(value) = input else {
        return None;
    };
    if from_id == "chest_in" || from_id == "bust_cm" {
        let cm = to_cm(*value, from_id);
        if !(75.0..=130.0).contains(&cm) {
            return None;
        }
        return nearest_row(MENS_TOPS, |r| Cell::Number(r.chest), cm);
    }

    None
}

fn convert_mens(input: &ClothingValue, from_id: &str, to_id: &str) -> Option<ClothingValue> {
    let row = resolve_mens_row(from_id, input)?;

    if let Some((_, column)) = MENS_UNITS.iter().find(|(id, _)| *id == to_id) {
        return Some(format_output(mens_cell(row, *column), to_id));
    }

    if to_id == "chest_in" || to_id == "bust_cm" {
        return Some(ClothingValue::Number(from_cm(row.chest, to_id)));
    }

    None
}

fn convert_hat(input: &ClothingValue, from_id: &str, to_id: &str) -> Option<ClothingValue> {
    let ClothingValue::Number(value) = input else {
        return None;
    };

    let row = if from_id == "neck_cm" {
        if !(48.0..=64.0).contains(value) {
            return None;
        }
        nearest_row(HAT_SIZES, |r| Cell::Number(r.head), *value)?
    } else if let Some((_, column)) = HAT_UNITS.iter().find(|(id, _)| *id == from_id) {
        nearest_row(HAT_SIZES, |r| hat_cell(r, *column), *value)?
    } else {
        return None;
    };

    if to_id == "neck_cm" {
        return Some(ClothingValue::Number(row.head));
    }
    let (_, to_column) = HAT_UNITS.iter().find(|(id, _)| *id == to_id)?;
    Some(format_output(hat_cell(row, *to_column), to_id))
}

fn convert_glove(input: &ClothingValue, from_id: &str, to_id: &str) -> Option<ClothingValue> {
    let ClothingValue::Number(value) = input else {
        return None;
    };
    let (_, from_column) = GLOVE_UNITS.iter().find(|(id, _)| *id == from_id)?;
    let row = nearest_row(GLOVE_SIZES, |r| glove_cell(r, *from_column), *value)?;
    let (_, to_column) = GLOVE_UNITS.iter().find(|(id, _)| *id == to_id)?;
    Some(format_output(glove_cell(row, *to_column), to_id))
}

const LINEAR_PAIRS: &[(&str, &str)] = &[
    ("inseam_cm", "inseam_in"),
    ("inseam_in", "inseam_cm"),
    ("bust_cm", "chest_in"),
    ("chest_in", "bust_cm"),
    ("waist_cm", "waist_in"),
    ("waist_in", "waist_cm"),
    ("hip_cm", "hip_in"),
    ("hip_in", "hip_cm"),
];

fn convert_linear(input: &ClothingValue, from_id: &str, to_id: &str) -> Option<ClothingValue> {
    let ClothingValue::Number(value) = input else {
        return None;
    };
    if !LINEAR_PAIRS
        .iter()
        .any(|(a, b)| *a == from_id && *b == to_id)
    {
        return None;
    }
    let cm = to_cm(*value, from_id);
    Some(ClothingValue::Number(from_cm(cm, to_id)))
}

fn convert_clothing_size_parsed(
    input: &ClothingValue,
    from_id: &str,
    to_id: &str,
) -> Option<ClothingValue> {
    if from_id == to_id {
        return Some(match input {
            ClothingValue::Label(l) => ClothingValue::Label(l.clone()),
            ClothingValue::Number(v) => {
                if is_letter_unit(to_id) {
                    ClothingValue::Label(format_number(*v))
                } else {
                    ClothingValue::Number(*v)
                }
            }
        });
    }

    if let Some(result) = convert_linear(input, from_id, to_id) {
        return Some(result);
    }

    let from_chart = chart_kind(from_id);
    let to_chart = chart_kind(to_id);

    match (from_chart, to_chart) {
        (Some(ChartKind::Womens), Some(ChartKind::Womens)) | (Some(ChartKind::Womens), None) => {
            convert_womens(input, from_id, to_id)
        }
        (Some(ChartKind::Mens), Some(ChartKind::Mens)) => convert_mens(input, from_id, to_id),
        (Some(ChartKind::Hat), Some(ChartKind::Hat)) => convert_hat(input, from_id, to_id),
        (Some(ChartKind::Hat), _) if to_id == "neck_cm" => convert_hat(input, from_id, to_id),
        (Some(ChartKind::Glove), Some(ChartKind::Glove)) => convert_glove(input, from_id, to_id),
        (Some(ChartKind::Womens), Some(ChartKind::Mens)) => {
            let interim = convert_womens(input, from_id, "bust_cm")?;
            let ClothingValue::Number(v) = interim else {
                return None;
            };
            if v == 0.0 {
                return None;
            }
            convert_mens(&ClothingValue::Number(v), "bust_cm", to_id)
        }
        (Some(ChartKind::Mens), Some(ChartKind::Womens)) => {
            let interim = convert_mens(input, from_id, "bust_cm")?;
            let ClothingValue::Number(v) = interim else {
                return None;
            };
            if v == 0.0 {
                return None;
            }
            convert_womens(&ClothingValue::Number(v), "bust_cm", to_id)
        }
        _ => None,
    }
}

/// Parses `raw` (a number or a size label) in `from_id` and converts to `to_id`. Returns
/// [`ConvertError::MalformedInput`] if `raw` can't be parsed at all (empty, or neither a valid
/// label nor a valid number), and [`ConvertError::UnsupportedFeature`] if parsing succeeds but no
/// conversion path exists — an honest-gap unit (kids'/bra sizing), an unrecognized unit id, or a
/// measurement outside this chart's supported range.
pub fn convert_from_input(
    raw: &str,
    from_id: &str,
    to_id: &str,
    format: Format,
) -> Result<ClothingValue, ConvertError> {
    let parsed = parse_input(raw, from_id).ok_or(ConvertError::MalformedInput { format })?;
    convert_clothing_size_parsed(&parsed, from_id, to_id).ok_or(ConvertError::UnsupportedFeature {
        format,
        feature: "no_conversion_data",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_chart_lookup_us_womens_to_international_letter() {
        // us_womens_numeric 8 -> row index 4 (bust 92) -> letter "S".
        let result = convert_from_input(
            "8",
            "us_womens_numeric",
            "international_letter",
            Format::PlainText,
        )
        .unwrap();
        assert_eq!(result, ClothingValue::Label("S".to_string()));
    }

    #[test]
    fn letter_label_round_trips_through_a_number_chart() {
        let result = convert_from_input(
            "M",
            "international_letter",
            "us_womens_numeric",
            Format::PlainText,
        )
        .unwrap();
        // "M" is int letter for both the (bust 96, us 10) and (bust 92, us 8) rows — TS
        // `find` picks the first match in table order, so this should be row bust=96 -> us 10.
        assert_eq!(result, ClothingValue::Number(10.0));
    }

    #[test]
    fn letter_alias_is_normalized() {
        let result = convert_from_input(
            "2XL",
            "international_letter",
            "international_letter",
            Format::PlainText,
        )
        .unwrap();
        assert_eq!(result, ClothingValue::Label("XXL".to_string()));
    }

    #[test]
    fn linear_measurement_pair_converts_cm_to_inches() {
        let result =
            convert_from_input("2.54", "inseam_cm", "inseam_in", Format::PlainText).unwrap();
        assert_eq!(result, ClothingValue::Number(1.0));
    }

    #[test]
    fn kids_sizing_is_an_honest_gap() {
        let err = convert_from_input("5", "us_kids", "eu_kids", Format::PlainText).unwrap_err();
        assert!(matches!(err, ConvertError::UnsupportedFeature { .. }));
    }

    #[test]
    fn bra_sizing_is_an_honest_gap() {
        let err = convert_from_input("34B", "bra_us", "bra_eu", Format::PlainText).unwrap_err();
        assert!(matches!(err, ConvertError::UnsupportedFeature { .. }));
    }

    #[test]
    fn empty_value_is_malformed() {
        let err = convert_from_input("", "us_womens_numeric", "eu_womens", Format::PlainText)
            .unwrap_err();
        assert!(matches!(err, ConvertError::MalformedInput { .. }));
    }
}
