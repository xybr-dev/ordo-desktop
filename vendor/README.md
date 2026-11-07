# Vendored crates

## gpui-pre-windows 0.3.8

An unmodified copy of the crates.io release, apart from two changes marked `ORDO PATCH`:

- **Display changes (Decision 308), `src/events.rs`.** `handle_display_change_msg` now calls the
  window's `moved` callback, including when the window is left on no monitor. That runs GPUI's bounds
  observers, so `crates/ordo/src/output.rs` hears about every display change.
- **Hidden pointer (Decision 313), `src/window.rs` and `src/events.rs`.** A window opened with
  `focus: false` records `hides_pointer`, and `handle_set_cursor` then sets no cursor. The
  congregation window is the only such window, so the pointer is hidden only while it is over the
  projector.

`cargo xtask verify` fails if either patch stops applying, because Cargo only warns about an unused
`[patch]` and would quietly restore the unpatched crate.

To upgrade GPUI, replace this directory with the new release, reapply both marked changes, update the
version in the root `Cargo.toml` pin, and retest display loss and the hidden pointer on real hardware.
