# GPUI behavior dependency qualification for T2.7 (TextArea)

Pins are unchanged from `GPUI-BASE-T0.4a.md`: `gpui-pre =0.3.8` and
`gpui-base =0.7.1`. T2.7 adds one more behavior use inside the `ui` crate:
`gpui_base::input::{Textarea, TextareaState}` behind `ui::TextArea`. No
`gpui-base` type crosses the `ui` Interface.

Each item is marked one of: **automated test** (a test in
`crates/ui/src/primitives/text_area.rs` fails if it regresses), **source read**
(checked by reading the pinned source, no test), or **NOT checked**.

## Finding that shapes the wrapper

`TextareaState` has three layouts. Tab inserts an indent in `PlainText` and
`CodeEditor`, and only propagates in `AutoGrow`. Ordo needs Tab and Shift+Tab to
leave the field (they move focus through `FocusNext` / `FocusPrevious`), so
`TextAreaState` always builds the base with `auto_grow(min, max)`. The base
notifies on every `set_auto_grow` and `set_placeholder`, so the render writes
them only when they changed (the same render-loop rule as `Field`).

## Items

| Item | Status | Evidence |
| --- | --- | --- |
| Offsets are UTF-8 bytes on char boundaries | automated test | `offsets_are_utf8_bytes_on_character_boundaries` ("Ginoó\nNiño") |
| `set_value` silent; native undo cannot restore older text | automated test | `set_value_is_silent_and_native_undo_cannot_bring_back_older_text` |
| Keyboard: type, delete, paste, Undo, Redo each emit one Changed | automated test | `changed_fires_once_per_edit_and_native_undo_is_an_edit` |
| Keyboard: Enter inserts a newline | automated test | `enter_inserts_a_newline_and_never_submits` |
| Keyboard: Tab and Shift+Tab leave the field | automated test | `tab_and_shift_tab_leave_the_field_without_editing_it` |
| Keyboard: Escape reaches an enclosing overlay | automated test | `escape_is_not_swallowed_so_an_enclosing_overlay_closes` |
| Focus: FocusLeft on focus moving away and on window deactivation | automated test | `focus_left_fires_when_focus_moves_elsewhere`, `window_deactivation_reports_focus_left_exactly_as_a_field_does` |
| AccessKit role, label, value, placeholder, description | automated test | `the_node_is_a_named_multi_line_text_input_with_its_value_and_reason` (reads the node the control writes, not a screen reader's output) |
| Clipboard copy and paste | automated test | paste in the Changed test, copy in the read-only test (test clipboard, not the OS clipboard) |
| Soft wrap, no ellipsis | automated test | `it_soft_wraps_and_never_ellipsizes` |
| Rows grow then scroll inside | automated test | `rows_grow_with_the_text_then_scroll_inside` |
| Large Interface scale (200%) wraps within its column | automated test | `wrapping_holds_at_the_largest_interface_scale` |
| Read-only: not editable, focusable, selectable, reason shown | automated test | `a_read_only_text_area_stays_focusable_selectable_and_says_why` |
| High Contrast focus ring | source read | the ring comes from `focus_rings(environment, surface)` as in `Field`; no test renders it under Windows High Contrast |
| IME composition | NOT checked | no test drives an IME; the base implements `EntityInputHandler` but composition was not exercised |
| Real screen reader announcement | NOT checked | only the AccessKit node is tested |
| OS clipboard | NOT checked | tests use the test platform clipboard |
| Packaged build on a church PC | NOT checked | needs the manual gate |
