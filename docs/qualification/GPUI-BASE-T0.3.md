# GPUI behavior dependency qualification for T0.3

## Decision

Qualify `gpui-base` `0.5.2` at revision
`7885c41663c7a6cc68ad0c99b1ba33550f807ff0` for selective use inside the
internal `ui` crate with GPUI revision
`8b1497dbd22fb06f5838a7c0b84a1e54fafa71bc`.

The package is an unstyled behavior layer licensed Apache-2.0. It is not Ordo's
visual language, application overlay owner, or public primitive Interface. No
`gpui-base` type may cross `ui`; replacing an adopted behavior changes only the
corresponding Ordo wrapper.

Any revision change, or any additional UI behavior dependency, invalidates this
qualification and requires the same pin, license, focus, accessibility, and
removal review before adoption.

## T0.3 behavior selection

| T0.3 need | Pinned behavior | Decision and boundary |
| --- | --- | --- |
| Button | `Button` | Adopt interaction, focus, keyboard activation, and base Button semantics behind an Ordo Button. Ordo owns every visual and semantic-state variant. |
| Field | `Input` and `InputBaseState` | Adopt native editing, selection, clipboard, keyboard, and focus behavior behind an Ordo Field. Reject the full code-oriented `Editor` for ordinary fields. |
| Tooltip | `Tooltip`, `TooltipOverlay`, `TooltipPositioner` | Adopt positioning and Tooltip role behind an Ordo Tooltip; the wrapper owns trigger association, wording, delay, and appearance. |
| Combobox | `Combobox`/`Select` | Adopt controlled open state, trigger/content focus transfer, and Up/Down/Enter/Escape dispatch. Ordo owns popup content, selection state, naming, and styling. |
| Disclosure | `Collapsible` | Adopt conditional content rendering only. Ordo supplies the Button trigger, expanded state, relationships, focus, and region semantics. |
| Tabs | `Tab` and `Tabs` | Adopt Tab/TabList roles, selected state, position metadata, and activation. Ordo owns roving focus, arrow keys, labels, and local-view styling. |
| Menu | none as a complete primitive | Reject `Popover` as the Menu root because it exposes a Dialog role. Build Ordo Menu semantics with direct GPUI; use base positioning only if it can remain an internal detail. |
| Dialog | `Dialog` focus/change mechanics | Adopt focus trapping, Escape/confirm behavior, and controlled change reasons behind the application overlay host. Ordo owns names, descriptions, scrim, viewport bounds, layout, and motion. |
| Slideover | `Sheet` focus/dismiss mechanics | Adopt focus trapping, Escape, and input-capturing outside-click mechanics. Reject standalone Sheet ownership: every slideover and nested slideover is rendered by Ordo's one overlay host. |
| Toast | `ToastStack` mechanics | Adopt stacking and transition bookkeeping. Ordo owns bottom-center placement and the typed distinction between expiring confirmation/Undo and persistent owned failure. |
| Focus treatment | direct GPUI | Reject base visual focus styling. Ordo owns the two-layer, keyboard-modality-aware focus treatment and the High Contrast escape hatch. |
| Motion | GPUI/native plus Ordo tokens | Reject base spring motion for T0.3 overlays; the design contract forbids spring, bounce, and overshoot. |
| Theme/styling | direct GPUI plus Ordo tokens | Reject `Theme`, `StyledExt` visual helpers, and the styled `gpui-component` façade as product vocabulary. |

Calling `gpui_base::init` is permitted only as infrastructure required by an
adopted behavior. It does not authorize another exported component or base
theme in production UI.

## Keyboard and focus evidence

Inspection of the pinned source and its tests established:

- `Button` owns focus and keyboard activation.
- `InputBaseState` uses a tab-stop `FocusHandle` and registers the native text
  editing, selection, clipboard, Undo/Redo, and navigation bindings.
- `Combobox` transfers focus between its trigger and content and binds
  Up/Down/Enter/Escape.
- `Dialog` and `Sheet` install focus traps and handle Escape; `Sheet` stops the
  outside pointer event before requesting dismissal.
- `Tabs` does not supply Ordo's required roving-focus/arrow policy, so the Ordo
  wrapper must own and test it.
- `Collapsible` only includes or omits content, so it cannot own a disclosure's
  trigger or focus behavior.

T0.3 primitive tests remain responsible for visible reading order, focus
return, modal containment, selection independence, and `F6` region traversal.

## AccessKit evidence and gaps

The pinned implementation supplies these useful semantics:

- `Button` writes a Button role, accessible label, and Click action only while
  enabled.
- `InputBase` defaults to TextInput semantics.
- `Select` exposes ComboBox semantics and accepts an application-owned name.
- `Tab`/`Tabs` expose Tab and TabList roles, selected state, set position, and
  activation.
- `Tooltip` exposes a Tooltip role; `Dialog` exposes Dialog semantics; `Toast`
  exposes Alert semantics.

These do not complete Ordo's acceptance gate. In particular, the pinned Button
test records that GPUI does not expose the disabled flag through its current
stateful-element setter; Collapsible supplies no expanded/relationship
semantics; and application-specific names, descriptions, values, availability,
focus, selection, and live-status policy remain Ordo-owned. Each wrapper must
capture and assert its final AccessKit node rather than assuming the base node
is sufficient.

## Dependency, source, and license evidence

- Root `Cargo.toml` and `Cargo.lock` pin both git revisions; `xtask` rejects a
  floating or mismatched source.
- `gpui-base` declares Apache-2.0 and ships `LICENSE-APACHE` at the pinned
  revision.
- `cargo deny check` evaluates the complete locked transitive graph for
  advisories, bans, licenses, and sources. Its explicit advisory exceptions and
  allowed git sources are reviewable in `deny.toml`; no dependency is silently
  omitted from that graph.
- `cargo xtask verify` enforces that `ui` has no unqualified direct foundation
  dependency and that no other workspace package imports `gpui-base`, including
  through dev-dependencies. Removing `gpui-base` remains permitted.
- The `cargo deny` bans reject the styled façade and common Rust browser/UI
  runtime stacks anywhere in the complete locked graph rather than relying on
  a hand-maintained package subset.

This is engineering qualification evidence, not a closed-source distribution
decision. A later distribution decision must retain required notices and review
the then-current locked graph; changing that graph reruns the same gate.
