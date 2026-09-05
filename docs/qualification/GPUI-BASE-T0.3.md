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
| Select | `Combobox`/`Select` | Adopt controlled open state, trigger/content focus transfer, and Up/Down/Enter/Escape dispatch. Ordo owns popup content, selection state, naming, and styling. |
| Combobox | the same `Combobox`/`Select` behavior plus the adopted `Input` | Adopt the same open/focus/dispatch behavior with an Ordo Field as the trigger. Ordo owns filtering, the caret/active-choice split, commit rules, and the changing visible choice set. |
| Disclosure | `Collapsible` | Adopt conditional content rendering only. Ordo supplies the Button trigger, expanded state, relationships, focus, and region semantics. |
| Tabs | `Tab` and `Tabs` | Adopt Tab/TabList roles, selected state, position metadata, and activation. Ordo owns roving focus, arrow keys, labels, and local-view styling. |
| Menu | none as a complete primitive | Reject `Popover` as the Menu root because it exposes a Dialog role. Build Ordo Menu semantics with direct GPUI; use base positioning only if it can remain an internal detail. |
| Dialog | `Dialog` focus/change mechanics | Adopt focus trapping, Escape/confirm behavior, and controlled change reasons behind the application overlay host. Ordo owns names, descriptions, scrim, viewport bounds, layout, and motion. |
| Slideover | `Sheet` focus/dismiss mechanics | Adopt focus trapping, Escape, and input-capturing outside-click mechanics. Reject standalone Sheet ownership: every slideover and nested slideover is rendered by Ordo's one overlay host. |
| Toast | `ToastStack` mechanics | Adopt stacking and transition bookkeeping. Ordo owns bottom-center placement and the typed distinction between expiring confirmation/Undo and persistent owned failure. |
| Focus treatment | direct GPUI | Reject base visual focus styling. Ordo owns the two-layer, keyboard-modality-aware focus treatment and the High Contrast escape hatch. |
| Motion | GPUI/native plus Ordo tokens | Reject base spring motion for T0.3 overlays; the design contract forbids spring, bounce, and overshoot. |
| Theme/styling | direct GPUI plus Ordo tokens | Reject `Theme`, `StyledExt` visual helpers, and the styled `gpui-component` façade as product vocabulary. |

The base package names its button-triggered popup list `Combobox`/`Select`
interchangeably. Ordo does not: an Ordo **Select** is a button trigger plus a
popup list, and an Ordo **Combobox** is an editable Field trigger plus a
filtered popup list. Both wrap the same base behavior, and both keep the
`ComboBox` AccessKit role the base root already exposes, which is correct for a
collapsed select either way. Base type names below are the package's, not
Ordo's.

Calling `gpui_base::init` is permitted only as infrastructure required by an
adopted behavior. It does not authorize another exported component or base
theme in production UI.

## Keyboard and focus evidence

Inspection of the pinned source and its tests established:

- `Button` owns focus and keyboard activation.
- `InputBaseState` uses a tab-stop `FocusHandle` and registers the native text
  editing, selection, clipboard, Undo/Redo, and navigation bindings.
- `Combobox`/`Select` transfers focus between its trigger and content and binds
  Up/Down/Enter/Escape, which backs both Ordo's Select and its Combobox.
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
- The base `Select` exposes ComboBox semantics and accepts an application-owned
  name.
- `Tab`/`Tabs` expose Tab and TabList roles, selected state, set position, and
  activation.
- `Tooltip` exposes a Tooltip role; `Dialog` exposes Dialog semantics; `Toast`
  exposes Alert semantics.

These do not complete Ordo's acceptance gate. In particular, the pinned Button
test records that GPUI does not expose the disabled flag through its current
stateful-element setter, so Ordo's consequentially blocked controls announce
that state through the accessible description instead and their tests assert
exactly that rather than support Ordo does not have. The same gap covers
read-only: AccessKit can represent it, but the pinned GPUI exposes no
`aria_read_only` setter either, so a Read-only or Unavailable Field states its
condition in the same description. Collapsible supplies no
expanded/relationship
semantics; and application-specific names, descriptions, values, availability,
focus, selection, and live-status policy remain Ordo-owned. Each wrapper must
capture and assert its final AccessKit node rather than assuming the base node
is sufficient.

## Test-harness gaps

### T0.3.6 composite-control evidence

`primitives::selection` owns Menu, select-only Combobox, and local-view Tabs;
`primitives::disclosure` owns Disclosure. No new dependency is adopted. The
Combobox uses the qualified `Select` root because it accepts the visible name;
popup key handling consumes confirmation before the base handler can return
focus to an option that selection has just closed.

The primitive tests drive real key-down/key-up and pointer events. They cover
provisional versus committed selection, skipped Disabled choices, focusable
but inert Unavailable choices, Home/End and wrapping arrows, Escape and Tab
dismissal, disclosure collapse, callback counts, and invoker focus return.
Final element-node probes assert Menu/MenuItem, ComboBox/ListBox/option,
TabList/Tab, and disclosure Button roles, labels, selected/expanded state, and
available versus blocked Click actions.

The lab registers `menu`, `combobox`, `tabs`, and `disclosure`. Its automated
matrix renders all four at both appearances, five scales, and both viewport
presets (80 combinations), opens both popup kinds, asserts window containment,
and scrolls the last multilingual value into view with End. The popup body
uses a bounded native scroll container and Floating elevation. The shared
application overlay host remains PR 9 work.

Collections are fixed for a state's lifetime; callers recreate state when
membership changes. Tabs expose local selection and a callback; the owning
view renders its named TabPanel (as the lab does). Disclosure renders a named
Region below its expanded Button. The pin has no public `aria_controls` or
`aria_labelled_by` relationship setters: matching names and nesting are
provided, but explicit AccessKit relationship edges and full tree shape are
not claimed by these element-node tests. Live assistive-technology and Windows
High Contrast checks remain the T0.3 manual gate.

### T0.3.7 editable-Combobox evidence

`primitives::selection` now also owns the editable Combobox. It adds no
dependency and no third selection implementation: the trigger is the Ordo Field
from T0.3.5, the popup is the Select's own list, and the root is the same
qualified `Select`, which is why both collapse to one `ComboBox` node.

The caret and the active choice are independent because focus never leaves the
Field. The base root would move it into the popup on Up/Down, so those two
arrive as `SelectUp`/`SelectDown` and are consumed in the capture phase before
the root's own handler; the active choice is then marked with
`aria_active_descendant` rather than focused. Enter and Escape need no such
interception: the pinned single-line input leaves both to propagate, so they
reach the root's `on_confirm` and `on_dismiss` seams, which commit and revert.
Tab is not bound in either context and is handled from the trigger's key-down.

Filtering breaks T0.3.6's "collections are fixed for a state's lifetime" only
for the *visible* subset. A choice is addressed by its index in the whole
collection everywhere — focus handles, selection, callbacks — and the filtered
view is derived per frame, so a choice keeps its identity across filters. What
the operator typed is a filter only while it names no choice; arriving at one
ends the filter, so the collection opens back up around it.

One rule settles both the selection and the open set: **a Combobox's selection
is whichever choice its Field names exactly, and none while the text names
none.** A half-typed value, a cleared one, and a name the collection does not
contain are all "no selection", so no row is filled, no caller reads a value the
operator cannot see, and a caller that accepts a new name reads
`SelectionState::text` and decides for itself. `MVP-PLAN.md` needs exactly that:
the Presider and Location comboboxes accept a custom name immediately, and the
Choir selector quick-creates an unmatched one. Enforcing membership is therefore
the caller's, not the primitive's — the primitive reports `None` and nothing
else. 
Reporting `None` is not enough on its own, because a popup that answers an
unmatched name with "No matches" reads as a refusal when the truth is that the
name may be kept. `SelectionState::accepting_new_values` therefore ends an
unmatched filter with an `Add "<typed>"` row. It is a row like any other —
arrows reach it, Enter takes it, the pointer clicks it — so it is addressed one
past the collection, and every lookup that can meet that index asks
`choices.get` instead of indexing. Nothing else hands the index out, and
`current` derives from labels, so it can never escape as a selection. Taking the
row commits nothing but the closing: keeping the typed name is what Tab already
did, and the row exists to say so. What a new name *means* — Choir's normalized
duplicate check, a Presider who never becomes a parish record — stays with the
caller, which reads `text`. A collection without that opt-in still states it has
no match, because a closed collection must not invite a value it will refuse.

Leaving and cancelling are therefore different. Escape cancels: the Field goes
back to what it held when the popup opened. Tab and a press outside just close,
keeping what was typed. An empty Combobox is the ordinary **Not assigned** value
rather than a choice that spells the words, so the collection needs no such
pseudo-choice; the Clear control inside the Field gives the value up, which is a
commitment rather than an abandoned filter, and reports `None` through the same
callback a choice reports through. A filter that matches nothing renders a
stated empty result rather than an empty popup.

`Element::write_a11y_info` does not carry the active-descendant claim: the pin
sets it through interactivity and resolves it at paint. The element-node tests
therefore assert the ComboBox root, its name and expanded state, and the
ListBox/option roles, and the behavior tests cover which choice is active.

The lab registers `combobox` and drives it through the same 80-combination
matrix: opening from the Field, filtering to one long multilingual match,
filtering to none, and reverting on Escape, with window containment asserted at
both viewports and every scale.

### Pinned platform limitations

The pinned revision also bounds what T0.3 can automate.

`observe_window_appearance` is the supported way to follow operating-system
appearance while Ordo runs, but no downstream crate can drive it in a test:
`TestWindow::simulate_appearance_change` exists at the pinned revision while
`TestAppContext::test_window` is `pub(crate)`, and the public `simulate_*`
surface covers resize, keystrokes, mouse, modifiers, prompts, and close only.

The accessibility tree is a second gap of the same shape. GPUI builds it only
while a platform adapter has activated it, and the test platform implements no
`a11y_init`, so `Window::debug_a11y_tree_json` is always `None` under
`TestAppContext`. T0.3 therefore captures a primitive's node from the element
itself — `Element::a11y_role` plus `Element::write_a11y_info` — which is what
the pinned `gpui-base` tests do and is enough to assert a final node's role,
name, description, and actions. What it cannot cover is tree shape, so
whole-window claims such as "no operator-only node reaches the congregation
window" still need a live adapter.

Ordo therefore automates the two halves it owns — the `WindowAppearance` to
resolved-appearance mapping, and the `SystemAppearanceChanged` transition — and
covers live following by the `[MANUAL]` check recorded in T0.3. Closing the gap
needs a revision that exposes its test window, which reruns this qualification
anyway.

The pin supplies no scroll indicator. `overflow_y_scroll` scrolls from wheel
and keyboard but paints nothing, and GPUI ships no scrollbar element, so a list
taller than its bounded owner gives no sign that more content exists below the
fold. Ordo therefore owns one thin overlay indicator drawn from
`ScrollHandle::offset()` and the content-versus-viewport height, shared by every
bounded scrolling surface rather than reimplemented per list. Adopting a base
scrollbar later reruns this qualification.

No platform mode is reported either. The pinned revision exposes no
high-contrast or forced-colors signal, and a GPU-composited GPUI window gets no
system repaint of its own controls, so there is no operating-system focus
indication for Ordo to defer to. Ordo therefore paints its focus treatment in
every mode, which is what `MILESTONES.md`'s `[MANUAL]` gate asks to be
preserved. Deferring to the platform becomes implementable only when a revision
reports the mode and its system colors, which reruns this qualification anyway.

## Dependency, source, and license evidence

- Root `Cargo.toml` and `Cargo.lock` pin both git revisions; `xtask` rejects a
  floating or mismatched source.
- `gpui-base` declares Apache-2.0 and ships `LICENSE-APACHE` at the pinned
  revision.
- `cargo deny check` evaluates the complete locked transitive graph for
  advisories, bans, licenses, and sources. Its explicit advisory exceptions and
  allowed git sources are reviewable in `deny.toml`; no dependency is silently
  omitted from that graph.
- `cargo xtask verify` qualifies only `ui`'s normal direct foundation
  dependencies, leaving test-only helpers outside that policy. A separate scan
  prevents every other workspace package from importing `gpui-base` through
  either normal or dev-dependencies. Removing `gpui-base` remains permitted.
- The `cargo deny` bans reject the styled façade and common Rust browser/UI
  runtime stacks anywhere in the complete locked graph rather than relying on
  a hand-maintained package subset.

This is engineering qualification evidence, not a closed-source distribution
decision. A later distribution decision must retain required notices and review
the then-current locked graph; changing that graph reruns the same gate.
