# @conv.cat/data

Static data: format catalogs, unit definitions, timezone tables, and i18n translation bundles.
License: **MIT**.

This is data, not logic. It must never contain conversion algorithms, parsing, or encoding —
those belong in `crates/conv-core`. Keep it declarative (JSON/TS data modules) so both
`apps/web` and `apps/desktop`, and eventually `crates/conv-core` itself where relevant, can
consume the same source of truth instead of drifting copies.

This is currently a scaffold: no catalogs, units, timezones, or translations have been ported
over yet.
