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
| Dialog | `Sheet` focus/dismiss mechanics | Adopt focus trapping, Escape, and the input-capturing outside press behind the application overlay host. Reject `Dialog` as the modal root: it writes a Dialog role onto a host div the application cannot name, and Ordo owns names, descriptions, scrim, viewport bounds, layout, and motion. |
| Slideover | the same `Sheet` mechanics | Adopt the same trapping and dismissal. Reject standalone Sheet ownership: every slideover and nested slideover is rendered by Ordo's one overlay host, which also hands out the layer priorities. |
| Toast | `ToastManager` bookkeeping | Adopt ordering, timeouts, same-id replacement, and exit coordination. Reject the `ToastStack` element, which is spring-driven and scales a collapsed stack. Ordo owns bottom-center placement and the typed distinction between expiring confirmation/Undo and persistent owned failure. |
| Focus treatment | direct GPUI | Reject base visual focus styling. Ordo owns the two-layer, keyboard-modality-aware focus treatment and the High Contrast escape hatch. |
| Motion | `motion::transition` plus Ordo tokens | Adopt the duration-and-easing value transition, which retargets from the value it is at and collapses to its target under `App::reduce_motion`. Reject `motion::spring` and every spring-driven element; the design contract forbids spring, bounce, and overshoot. |
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
  outside pointer event before requesting dismissal. Installing a trap is all
  either does: `focus_trap` registers the container with a global manager, and
  the only code that consults it and cycles Tab is the styled façade's `Root`,
  which Ordo rejects — so the Tab policy is Ordo's, from the same
  `active-trap` shape.
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
reimplements it. Because there is only the one implementation, two indicators
can be on screen at once — an overlay body behind the popup it opened — so the
surface lends the indicator its own id, and both the element id and the debug
selector carry it. A test names the indicator it means.

`Scroll` replaces the bare `ScrollHandle` a surface used to own, because the
indicator needs one thing GPUI's handle does not keep: what the last frame drew.
A surface is measured during prepaint, after every render has run, so the frame
that opens a popup renders before its own geometry exists, and GPUI refuses to
dirty a window mid-draw. The surface therefore checks itself once the frame is
over and asks for one more only when the measurement moved under it. That
converges in a single extra frame, leaves an idle surface idle, and — this is
the half a first-frame-only fix would miss — takes the indicator away again when
filtering shrinks the content back inside its box.

The check waits on a task rather than `Window::on_next_frame`, which is the same
shape and would be cheaper per frame. `on_next_frame` wakes the platform frame
source to deliver its callback, so registering one every render asks for the
next frame unconditionally and an open popup repaints forever — losing the
idleness the check exists to keep.

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
indicator is a thin overlay that starts exactly where the popup's corner leaves
off and advances when `End` scrolls to the long last value; the Combobox
filtered to its one match asserts the indicator is gone. `End` bounds the far
end rather than pinning it, because scrolling a choice into view stops at that
choice and leaves the list's own bottom padding as travel remaining. The far
end is pinned by the test that drives the pointer instead, where the offset is
clamped to the surface's maximum: the indicator widens as it is reached for, a
real press-move-move-release drags the surface to the end of its collection and
lands the indicator on the end of its track, and the drag is not mistaken for
the press outside that dismisses a popup. Between them the two assertions check
the derivation the length and the drag gain both rest on — that the track is the
surface's own measured bounds inset by its corner at each end.

### T0.3.9 overlay-layer evidence

`ui::overlay` owns every temporary surface: dialogs, slideovers, nested
slideovers, and toasts. It adds no dependency, and it is the only stacking
system — a view renders `OverlayHost::layer` once, last, and nothing else
defers anything of its own.

The modal root for both surfaces is `Sheet`, not `Dialog`. The two are the same
mechanics — a focus trap, Escape through the base `Cancel` action, and an
outside press that stops propagating before it asks to close — and the choice
between them came down to semantics: `Dialog` writes its `Dialog` role onto a
host div that takes no application name, and the pin exposes no way to name it
from inside. `Sheet` writes no role, so the node an operator's assistive
technology reads is Ordo's own surface, carrying the role, the visible title as
its name, and the complete text as a description. What `Dialog` had and `Sheet`
does not is Enter-confirms and its own layer priorities. Neither is missed:
`DESIGN.md` asks for no Enter-confirms — Enter on the focused primary action is
already the path — and this host hands out its own priorities, which it must do
anyway. A Select opened *inside* an overlay body has to paint over the surface
that owns it, so overlays sit below `POPUP_PRIORITY` and toasts between them,
above the surface a toast reports on and below a live control.

Tab is the half of a focus trap the pin does not implement. `focus_trap`
registers the container with a global manager and stops there; the only code
that reads that manager and cycles focus back inside is the styled façade's
`Root`, which this crate rejects. GPUI itself binds neither Tab nor Shift+Tab,
and the pin binds `tab` only in the input context, where a single-line Field
propagates it straight back out. `ui::init` therefore binds both to Ordo
actions, and the overlay layer answers them by stepping normally and then
continuing to step until focus is back inside its own trap. That is why the
binding lives in `ui` rather than in either root view: a modal that cannot hold
Tab is not a modal, so it could not wait for T0.3.10's region registry.

Motion is `motion::transition`, the base's duration-and-easing counterpart to
the spring this qualification rejects. It gives three things at once: the `400ms`
enter and `200ms` exit `DESIGN.md` names, an interruption that reverses from the
value the surface is at rather than from a new curve's start, and reduced motion
that *removes* the movement rather than shortening it, because the function
collapses to its target under `App::reduce_motion`. The exit is played rather
than skipped, so a surface stays mounted for its own exit duration and is taken
away by a task that first checks it is still the surface that was closing — a
replacement opened during that exit is unaffected, which is what `DESIGN.md`
means by motion never delaying the next input. One property of `transition`
shapes the code: it adopts its first target on the spot, so a surface whose
first frame asked for its resting value would simply be there. The frame that
opens a surface seeds the transition at zero first.

A dialog also rises one spacing step into its fade and falls back through it on
the way out, which is the pin's answer to the scale a web dialog grows through.
GPUI's `Style` carries opacity but no transform: the only transformation on the
pin is `svg`'s, and the base's own effect vocabulary — slide, fade, width, height
— is that same limit written down. Animating width and height would be a real
scale and would rewrap every line of the body on every frame of it, which costs
more than the flourish returns. The rise is a relative inset, a correction
applied after layout, so it moves nothing else and reflows nothing; it rides
`progress` itself, so the fade and the movement cannot drift apart.

`App::reduce_motion` is settable and readable but has no platform source at the
pin: nothing maps an operating-system `prefers-reduced-motion` onto it. Ordo
honors the flag wherever motion is drawn, and following the platform setting
becomes implementable when a revision reports it, which reruns this
qualification. The `easing` token is a CSS cubic bezier and the pin ships no
bezier sampler, so `ease_out_cubic` and `ease_in_cubic` stand in for it across
the enter and the exit.

A toast moves the same way, and for the same reason: it arrives one control
height below where it rests and falls back through on the way out, in the
shadcn toaster's own `400ms` and `200ms`, without the scale that toaster
applies. The manager's exit duration carries it, because that duration is a
mounting deadline — it is what keeps a leaving toast on screen long enough to
be seen leaving — while the enter duration stays zero, since a toast is present
the moment it is posted and only its drawing is timed. An exit ends by
unmounting rather than by an input, so the host wakes once more while anything
is still ending, which is the only clock it keeps.

Toasts adopt `ToastManager` and reject the `ToastStack` element, which is
spring-driven and scales a collapsed stack — both forbidden. The manager is pure
bookkeeping, and Ordo keys it by the message rather than by a counter, which is
what makes the channel deduplicated: the same operational change reported twice
replaces its own toast instead of stacking a second copy, and a caller whose
failure has been resolved takes it back by the words it posted. Its transition
durations are zero because a toast is a cut and only its expiry is timed. A
confirmation carries Undo and expires; a failure carries Dismiss and waits.
`Role::Status` and `Role::Alert` are the polite and interrupting halves
`DESIGN.md` distinguishes.

Two things the pin leaves open are settled by one GPUI mechanism, `occlude`,
which stops the hit test at a hitbox so nothing behind it is hovered, lends a
cursor, or is offered the wheel. The scrim uses it because `DESIGN.md` gives it
the pointer entirely: a control under an overlay that still lit up and still
showed a pointer cursor would read as something the operator can reach past.
`ui::scroll::Scroll::bounded` uses it because GPUI's own wheel listener never
stops propagating — every scrollable ancestor under the pointer scrolls by the
same delta, so one wheel over a popup inside a slideover body inside the
workspace moved all three. Every bounded scrolling surface now goes through
`bounded` rather than repeating `track_scroll`, `overflow_y_scroll`, and
`occlude`, so none of them can be the one that forgets.

The scrim is Ordo's, drawn in the `blackout` neutral faded by its own surface's
progress, and only the card occludes. That distinction is load-bearing rather
than cosmetic: a centring wrapper that took presses would be a second,
invisible scrim over the whole viewport, and the real one would never see the
press that dismisses. The body is the third bounded scrolling surface to use
`ui::scroll`, after T0.3.6's popup and T0.3.7's; it passes a corner of `0`,
because the rounded corners belong to the header and the footer above and below
it. Sizing it took the same care: `flex_1` and a `size_full` scroller both
measure zero, which left a dialog exactly its header plus its footer, so the
body measures from its own content and shrinks from there.

The lab registers `dialog`, `slideover`, and `toasts` against one keyed host,
which is how a scenario and the frame that paints the layer reach the same
stack without either being handed the other — the production shell consumes it
the same way. The overlay matrix opens both surfaces at both appearances, five
scales, and both viewport presets, asserting exterior gutters, a Close control
that stays in the header, a body indicator that is a thin overlay between the
header and the footer, and for the dialog a footer whose primary action ends it;
the slideover opens its nested child inside each combination and closes it
again with its parent still where it was. The behavior tests cover Escape and
the scrim closing with focus returned to the invoker, a scrim press activating
nothing beneath it, Tab and Shift+Tab cycling inside the trap, a nested child
returning focus to its parent, the enter that advances from the right and comes
to rest, reduced motion removing both the movement and the wait, and the
surface's final element node.

`ui::scroll::Scroll::indicator` now takes a `&str` rather than a `&'static
str`: an overlay body names itself after the surface it belongs to, and two
bodies can be on screen at once. `Scroll::bounded` joins it as the other half of
what a bounded surface asks for, and the Select/Menu/Combobox popup adopts it
too — it was the surface the containment bug was reported against.

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
reimplemented per list. It also has to own the repaint: the pin measures a
surface during prepaint and refuses to dirty a window mid-draw, so the indicator
would otherwise be a frame behind whatever it reports. Adopting a base scrollbar
later reruns this qualification.

`App::reduce_motion` is a third gap of the same shape, in the other direction:
the flag exists and is both readable and settable, but nothing at the pin maps
an operating-system `prefers-reduced-motion` onto it. Ordo honors it wherever
motion is drawn and automates that; following the platform setting needs a
revision that reports it, which reruns this qualification.

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
