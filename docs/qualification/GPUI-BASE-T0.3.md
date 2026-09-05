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

Home and End are deliberately *not* intercepted, which is the one place a
Combobox parts company with the Select it shares a popup with. They are bound
in the input context and move the caret to the ends of the Field, so the
operator correcting the front of a name they mistyped keeps the correction
instead of losing it to a jump to the first choice — the editable-combobox
behavior, where the ends of the collection are reached by arrowing. They are
not dead keys: `home_and_end_move_the_caret_and_leave_the_collection_where_it_was`
inserts a character after each and asserts where it lands, and that the popup
and the active choice are untouched.

Filtering breaks T0.3.6's "collections are fixed for a state's lifetime" only
for the *visible* subset. A choice is addressed by its index in the whole
collection everywhere — focus handles, selection, callbacks — and the filtered
view is derived per frame, so a choice keeps its identity across filters. What
the operator typed is a filter only while it names no choice; arriving at one
ends the filter, so the collection opens back up around it.

One rule settles both the selection and the open set: **a Combobox's selection
is whichever choice its Field names, and none while the text names none.** The
name has to be whole, but not in the operator's exact case, because the filter
already folds case and the two halves must agree: `english` filtering to
`English` while reporting no selection would offer to add the choice it is
showing, and hand the Choir or Presider caller its own duplicate to reject. Two
choices differing only in case are therefore one name, as they are to the
operator reading the list; a collection with genuinely duplicate labels resolves
to the first, because the Field's text is all this control has to go on.
Whenever the control settles closed on a name it matched — Tab, an outside
press, Escape, or a pick through the popup — the Field is rewritten to that
choice's own spelling, so the collapsed control shows the value it holds rather
than a near-miss of it, and `text` never hands a caller the operator's casing
for a name the parish already keeps. Typing itself is untouched, so no caret is
yanked mid-word, and a name outside the collection settles exactly as written.
A half-typed value, a cleared one, and a name the collection does not
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

That rule is also what makes the selection callback partial. `on_select`
reports a selection made *through the control* — a choice taken from the popup,
or Clear — while typing a name whole, and leaving with Tab or a press outside,
change the value without passing through it. Reading `selected` and `text` while
rendering is the authoritative path, and the state notifies, so an owning view
already redraws; the callback is for the side effects a deliberate pick should
have. Settling that now matters because T3.10's Choir selector and the
Presider/Location fields would otherwise each answer it differently.

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

The popup is the second bounded scrolling surface Ordo owns, after T0.3.6's,
and the scroll indicator `MILESTONES.md` requires did not exist when this
primitive landed — the pin paints none, as recorded above. T0.3.8 closes that
debt with the shared implementation described below, which the popup now
carries.

The lab registers both halves — `combobox`, a closed Language set, and
`combobox-new`, an open Presider set — bringing the same matrix to 120
combinations. Between them they cover opening from the Field, filtering to one
long multilingual match, filtering to none, the offered name and taking it with
the pointer, and cancelling with Escape, with window containment asserted at
both viewports and every scale. `ui-lab <scenario>` opens one by name, because
reviewing a control by hand should not mean cycling `F2` to reach it.

### T0.3.8 shared scroll-indicator evidence

`ui::scroll` owns the one indicator. It adds no dependency: the pin paints
nothing and ships no scrollbar element, so the geometry comes from
`ScrollHandle::bounds()`, `max_offset()`, and `offset()` and nothing else. The
Select, Menu, and Combobox popup is its first consumer; PR 9's overlay bodies
and T0.4's Cue timeline and dense editor lists are the next, and none of them
reimplements it.

`Scroll` replaces the bare `ScrollHandle` a surface used to own, because the
indicator needs one thing GPUI's handle does not keep: what the last frame drew.
A surface is measured during prepaint, after every render has run, so the frame
that opens a popup renders before its own geometry exists, and GPUI refuses to
dirty a window mid-draw. The surface therefore checks itself once the frame is
over and asks for one more only when the measurement moved under it. That
converges in a single extra frame, leaves an idle surface idle, and — this is
the half a first-frame-only fix would miss — takes the indicator away again when
filtering shrinks the content back inside its box.

The indicator is a sibling of the scrolling element inside a `relative` wrapper,
never its child: a child is translated by the scroll offset and would slide away
with the content it reports on. It is drawn in the strong border token over the
scrolled content, half a spacing step off the edge and with no reserved gutter.
Sitting on the edge means it has to stop where that edge curves away from it, so
its track is inset by the surface's own radius at both ends and the surface says
what that radius is — the popup's is `OVERLAY`, and a square pane passes `0`.
Position and length are both resolved in that track's own pixels. Keeping them
in one space is what makes staying inside it true rather than usually true: a
collection long enough to bind the minimum length, which the popup's six choices
never are and T0.4's Cue timeline easily will be, was otherwise positioned as
its unclamped fraction and ended up below the track by exactly what the minimum
had added. A track shorter than the minimum holds the indicator anyway.

It is draggable, which `DESIGN.md` allows: the rule is that it is never the
*only* way to scroll, and wheel, keyboard, and `scroll_to_item` behavior are
untouched. Reconciling a grabbable control with a bar thin enough to sit over
content takes two elements. A pointer-detecting strip along the edge widens the
bar to a grabbable width when the pointer comes near it; only the bar itself
blocks a press, so a row underneath loses nothing but the few pixels the bar
actually covers. Hover is counted on both, because the bar occludes the strip
beneath itself and a single flag would drop — and the bar shrink out from under
the pointer — at the moment the pointer arrived on it.

The drag is delta-based from where it took hold, so the indicator keeps the
point it was grabbed by rather than jumping its middle to the pointer, and the
pointer covers the track the bar has left to travel rather than the whole track,
so a drag to the bottom lands on the bottom of the content. GPUI only reports
moves outside an element's own bounds while a drag payload is active, which is
what `on_drag` is for here — the pointer leaves a four-pixel bar immediately and
has to keep dragging it anyway. The pointer says as much: the bar is an open
hand until it is taken hold of and a closed one while it is being dragged.

The indicator carries no AccessKit node: a scroll position is not something to
announce, and it adds no keyboard obligation because it adds no keyboard
behavior.

Its proportions are asserted directly: how much of the content is visible, where
that sits after scrolling, that an overscrolled offset stays inside its track,
and that content which fits yields nothing. The lab's existing matrix carries
the rest, so no scenario was added for it: the Menu and Select popups exceed
their bounded height at every appearance, scale, and viewport, and assert the
indicator is a thin overlay inside the popup and that it advances when `End`
scrolls to the long last value; the Combobox filtered to its one match asserts
the indicator is gone, and both ends of its travel stay clear of the popup's
corners. One further test drives the pointer: the indicator widens as it is
reached for, a real press-move-move-release drags the surface to the end of its
collection, and the drag is not mistaken for the press outside that dismisses a
popup.

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
fold. Ordo therefore owns one thin overlay indicator, `ui::scroll`, drawn from
the handle's bounds, maximum offset, and offset and dragging through
`set_offset`, shared by every bounded scrolling surface rather than
reimplemented per list. It also has to own the
repaint: the pin measures a surface during prepaint and refuses to dirty a
window mid-draw, so the indicator would otherwise be a frame behind whatever it
reports. Adopting a base scrollbar later reruns this qualification.

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
