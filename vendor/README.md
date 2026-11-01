# Vendored crates

## gpui-pre-windows 0.3.8

An unmodified copy of the crates.io release, apart from one change marked `ORDO PATCH` in
`src/events.rs` (Decision 299). `handle_display_change_msg` now calls the window's `moved` callback,
including when the window is left on no monitor. That runs GPUI's bounds observers, so
`crates/ordo/src/output.rs` hears about every display change.

`cargo xtask verify` fails if the patch stops applying, because Cargo only warns about an unused
`[patch]` and would quietly restore the unpatched crate.

To upgrade GPUI, replace this directory with the new release, reapply the marked change, update the
version in the root `Cargo.toml` pin, and retest display loss on real hardware.
