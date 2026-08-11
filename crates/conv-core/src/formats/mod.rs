//! Real conversion algorithms live under here, one submodule per [`crate::Category`]
//! (`units`, `text`, `cad`, `image`) — see `docs/adding-a-format.md` for the convention and a
//! full worked example. [`units`] is the first one to land (a representative 8-category subset —
//! see its own module docs); `text`/`cad`/`image` are still open backlog tickets.
//!
//! [`identity`] is the odd one out: a placeholder [`crate::Converter`] that proves the
//! [`crate::Registry`]/[`crate::convert`] pipeline end to end, not a real format converter — see
//! its own module docs.

pub mod identity;
pub mod units;
