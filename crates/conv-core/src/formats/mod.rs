//! Real conversion algorithms live under here, one submodule per [`crate::Category`]
//! (`units`, `text`, `image`) — see `docs/adding-a-format.md` for the convention and a
//! full worked example. [`units`] was the first to land (a representative 8-category subset —
//! see its own module docs); [`image`] is next, covering BMP ⇄ QOI so far (see its own module
//! docs for why those two formats specifically); `text` is still an open backlog ticket.
//!
//! [`identity`] is the odd one out: a placeholder [`crate::Converter`] that proves the
//! [`crate::Registry`]/[`crate::convert`] pipeline end to end, not a real format converter — see
//! its own module docs.

pub mod identity;
pub mod image;
pub mod units;
