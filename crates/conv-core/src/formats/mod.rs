//! Real conversion algorithms live under here, one submodule per [`crate::Category`]
//! (`units`, `text`, `cad`, `image`) — see `docs/adding-a-format.md` for the convention and a
//! full worked example. None of those category modules exist yet; format-by-format backlog
//! tickets add them one at a time.
//!
//! [`identity`] is the one exception: a placeholder [`crate::Converter`] that proves the
//! [`crate::Registry`]/[`crate::convert`] pipeline end to end before any real format lands. It is
//! not a real format converter and should not be imitated — see its own module docs.

pub mod identity;
