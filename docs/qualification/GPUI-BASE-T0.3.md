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

One `Sheet` is mounted per surface, and only the topmost surface that is not
already leaving is a modal: the others keep their shape but give up the press
that lands outside them and the capture that holds the pointer away from
everything behind, through `overlay_closable` and `overlay_interactive`. They
are made inert rather than unwrapped, because unwrapping moves the controls
inside a surface to a different place in the element tree, and at this pin an
element's identity is its path — a control that moves loses its keyed state and
the focus handle with it, which showed up as focus landing nowhere the moment a
nested slideover started to leave. A surface stops being the operator's when it
starts leaving, not when it finishes: focus goes back to the invoker at that
moment, and the card stops occluding, because `DESIGN.md` will not let motion
delay the next input and an exit is motion.

A control on a card playing its exit is a picture of a control, and the pin has
nothing that makes it one: opacity gates neither the hit test nor a listener,
and `occlude` was never what made the buttons inside a card live. So every
control that ends a decision goes through one guard that asks whether the
surface is still the operator's before it runs anything — the primary, the
secondary, and a toast's Undo. A decision taken twice is the delete run twice,
and `MVP-PLAN.md` puts a real deletion behind exactly this dialog.

Tab is the half of a focus trap the pin does not implement. `focus_trap`
registers the container with a global manager and stops there; the only code
that reads that manager and cycles focus back inside is the styled façade's
`Root`, which this crate rejects. GPUI itself binds neither Tab nor Shift+Tab,
and the pin binds `tab` only in the input context, where a single-line Field
propagates it straight back out. `ui::init` therefore binds both to Ordo
actions, and the overlay layer answers them by stepping normally and then
continuing to step until focus is back inside its own trap, ending on the
surface itself if the walk never found its way back — a walk that gave up
where it stood would be a modal that leaks Tab in the one case it exists for. That is why the
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

The dim is the layer's rather than each surface's. `DESIGN.md` gives every
overlay and its scrim one application-owned layer and says surfaces do not grow
more dramatic with nesting; a scrim per `Sheet` would put three nested
slideovers behind `0.83` of a `0.45` dim. One scrim, drawn directly under the
topmost open surface, fades in when the first surface opens and out when the
last one leaves, and stops taking the pointer as soon as everything on screen
is leaving.

A toast moves the same way, and for the same reason: it arrives one control
height below where it rests and falls back through on the way out, in the
shadcn toaster's own `400ms` and `200ms`, without the scale that toaster
applies. The manager's exit duration carries it, because that duration is a
mounting deadline — it is what keeps a leaving toast on screen long enough to
be seen leaving — while the enter duration stays zero, since a toast is present
the moment it is posted and only its drawing is timed. An exit ends by
unmounting rather than by an input, so the host wakes once more while anything
is still ending, which is the only clock it keeps.

The toasts a new one arrives under ride up rather than being redrawn a row
higher between two frames. What that costs is a measurement: a toast is as tall
as its own words and its own controls, and no token says how tall that is. GPUI
reports it through `on_children_prepainted`, one frame after the fact, so a
toast that has never been measured is taken out of the flow for that one frame —
it is at zero opacity anyway — and joins the stack on the next one, by which
time the toasts above it know exactly how far they are about to be pushed and
start from where they already were. A leaving toast gives that row back the
other way, by taking the space above it away as it fades, so the toasts it was
holding up ride down over the whole exit instead of dropping when it unmounts.
This is the part of the shadcn toaster's
stack motion that survives having no transform; the collapsed, scaled stack it
also draws is the part `ToastStack` was rejected for.

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

### T0.3.10 region-navigation and semantic evidence

`ui::accessibility::RegionRegistry` owns the ordered visible region cycle.
Callers supply only mounted regions in reading order; the registry retains each
region's focus handle and a weak remembered descendant under its stable visible
name. Names must be unique within a render and come from a fixed vocabulary,
not entity-derived titles: entries intentionally remain until the registry is
dropped, so hidden regions retain their remembered targets. A debug assertion
rejects duplicate names before their focus handles are mounted twice.

The registry hosts the ordinary Tab/Shift+Tab handlers, which walk GPUI's
whole-window tab order across region boundaries. F6/Shift+F6 instead walk only
the supplied visible regions. Workspace controls must descend from this root to
receive those handlers, which T0.3.11's Preparation shell does.
A focus-exit subscription also remembers departures by Tab or pointer. The
subscription belongs to the region and is dropped with its workspace registry.
Returning checks membership in the rendered focus tree, so a remembered control
that was removed falls back to the first surviving tab stop (or the region
itself when empty). Retaining a handle alone does not make a hidden control a
valid target. Neither F6 departures nor the focus-exit subscription remember
the region container itself: an empty region must try its first tab stop again
when content appears.

Mount the shared overlay layer as a sibling of the registry root, as the lab
scenario does. A dialog or nested slideover's focused dispatch path therefore
cannot reach the workspace F6 handlers, so F6 leaves its current control in
place. The tests prove this sibling composition, not an overlay F6-consuming
handler; the redundant no-op handlers were removed. PR 11 must preserve this
composition. Existing modal Tab containment and invoker restoration remain the
same shipping path.

The `regions` lab scenario uses the production registry, Button, Field, and
OverlayHost. It covers Global navigation, Workspace actions, Primary list,
Main editor, and an Inspector that can be hidden. Selecting its fixture item
updates contextual content without moving focus. It joins the existing lab
appearance/scale/viewport matrix. The keyboard test uses real key down/up
pairs, including Enter's raw Button handler. Its window is explicitly active:
the pin's `Window::draw` only sends focus-exit notifications for an active
window, so an inactive test window cannot exercise remembered Tab departures.

Automated evidence:

- `visible_regions_restore_targets_without_stealing_selection_focus`: Tab and
  Shift+Tab across region boundaries, F6/Shift+F6 restoration, hidden Inspector
  skipping, wraparound, selection without focus theft, dialog containment and
  return, and F6 from an editable Field.
- `a_removed_remembered_control_falls_back_to_the_surviving_region`: a retained
  but unmounted control is rejected. Removing the rendered-tree membership
  check was observed to fail this regression test before restoring it. The
  same test now covers an empty region gaining content after both F6 and
  direct focus departures; this failed before excluding container targets.
- `a_nested_slideover_keeps_its_parent_and_returns_focus_to_it`: additionally
  checks forward/reverse F6 while the deepest surface owns focus in the
  sibling composition above.
- `an_expired_confirmation_can_announce_the_same_message_again`: duplicate
  mounted confirmations remain one toast, expiry clears the current matching
  status, and a later identical message becomes a fresh node-content change.
  Expiry cannot clear a different current status. The test failed before the
  expiry reset was added.
- Captured node tests cover Region names/shortcuts, Tooltip role/full name,
  Field name/value/placeholder/description, Status and Alert roles and explicit
  live priorities, and consecutive announcement deduplication. Existing Button,
  Menu, Select, Combobox, Tabs, Disclosure, and overlay node captures continue
  to cover their actions, selection, expansion, values, and descriptions.

Ordo's Tooltip view now supplies its own Tooltip role and accessible name; it
had replaced the base view without retaining that node. Field supplies the
current value and placeholder to its TextInput frame. The previously qualified
Disabled/Read-only description fallbacks and missing relationship setters
remain as documented above.

`OverlayHost::announce_status` is the single polite status seam for expected
UI transitions, also used by confirmation/Undo toasts. Repeating the current
message leaves its node unchanged; a different intervening transition allows
the earlier message again. Unmounting a confirmation/Undo toast clears its
matching current announcement, so a subsequent identical confirmation can be
announced again after expiry or dismissal. The lab exercises Inspector
shown/hidden. Ordinary focus and selection never post to this channel. Confirmation toast nodes remain
inspectable but have live priority Off, avoiding a second announcement. Owned
failure toasts retain Alert semantics and use Assertive priority.

The pin exposes no live-priority setter on Div, and AccessKit consumer's
`Node::live` inherits Off when unspecified rather than inferring a priority
from Status/Alert roles. A private `LiveElement` delegates Stateful<Div>'s
layout, prepaint, paint, ID, and existing semantic writer, then sets only the
node's live priority. This adapter can disappear when GPUI exposes the setter;
no dependency or native adapter changes are needed.

Validation: `cargo xtask verify` passed all nine gates on 2026-09-06, including
84 UI tests, workspace tests, Clippy, release configuration, and cargo-deny.

These are element-node captures, not a running screen-reader test. The platform
limitations below still apply. Source inspection confirms that the passive
`CongregationView` renders neither the registry nor the operator overlay/status
host. Native tree isolation, spoken announcements, and Windows High Contrast
remain manual qualification. In particular, check with a live screen reader
that an icon-only control's tooltip speaks its full name once: its Tooltip
node has an explicit label and also contains visible text, and element-only
captures cannot establish whether a consumer repeats that subtree. Presenter
safety transitions and shell composition remain with their later owning tasks. This evidence does not close all of T0.3.

### T0.3.11 production-shell evidence

`crates/ordo/src/shell/` owns application workflow. `ui` keeps only reusable
primitives, the region registry, the overlay layer, and the development-only
lab, and it no longer holds a view of its own: the T0.1 prototype `RootView` is
replaced by `ordo::shell::ShellView` rather than grown. The actions split the
same way. `ui` declares only what it binds or answers — the Interface scale
keys, and the Tab/F6 pair the registry handles — while navigation, appearance,
and the Presenter transition are `ordo::shell`'s, because a destination is
application vocabulary and `ui` has no destinations. No dependency is added.

The registry's `render` is replaced by `region` and `root`, and the lab moves
with it. One call that both wrapped and placed every region could only produce
a flat list of siblings, which no workspace is: the rail, the header, the list,
and the editor are consecutive in reading order and nested in layout — the rail
beside a column, the header above a row of panes. `region` now wraps one region
in reading order and hands the element back for the caller to place; `root`
takes what was wrapped since the last call and hosts the handlers over it. It
must be their ancestor, which is the one thing the flat version enforced for
free and the doc comment now says.

`RegionRegistry::focus_region` is the other half, and it is what a focused
editor needs: the shell asks a region for the keyboard, and the registry
answers with the remembered-target-or-first-tab-stop walk the F6 cycle already
uses. It cannot answer immediately. `Window::focus_next` reads the *last
rendered* frame's tab stops, and every caller is a view that is changing what is
mounted, so the request is recorded and performed by a task the next `root`
spawns — the same "check the frame once it is over" shape T0.3.8 uses, for the
same reason. `Window::on_next_frame` is the obvious alternative and was tried:
registered from an event handler it never delivered under `TestAppContext`, and
it is in any case bound to an arbitrary next frame rather than to the render
that changes the tree.

Returning from a focused editor has to go through that seam rather than through
the invoker's own handle, because at this pin the handle does not survive.
`gpui_base::Button` takes its focus handle from `Window::use_keyed_state`, which
exists only while its element is rendered in consecutive frames, so a control
that is unmounted for the duration of a focused editor comes back a different
control. What the region can promise is the region: focus returns to the
control the operator left from when it survived, and to the region's first tab
stop otherwise. The test asserts the promise behaviorally — after Back, `Enter`
alone reopens the editor, so the keyboard is on the control that opened it.

The rail collapses from effective width and Interface scale, never from a
resolution breakpoint: it gives up its labels at the point where the remaining
workspace would fall below `DESIGN.md`'s `240px` list plus `520px` editor plus
its gutters, with both terms scaled and the window measured through
`Window::viewport_size`. The gutter reserved is the wide one, because that is
what the branch being decided — an expanded rail — actually renders; measuring it
against the compact gutter would leave the workspace narrower than the minimum
the threshold exists to protect. That threshold puts both rail states inside the
qualified matrix — `1366x768` collapses at 150% and `1920x1080` at 200% — so
neither state is a case only a hand-resized window would reach. Collapsed, a
destination is an icon whose tooltip is its accessible name, which is the
Button's own contract for an icon-only control. The active destination is
marked by a filled bar on the leading edge as well as the selection tint, so
`MILESTONES.md`'s non-color-only requirement does not rest on the fill.

Local view state is the view's rather than the window's. A destination keeps the
local view it was left on, and `Window::use_keyed_state` cannot: the tabs
unmount when the operator leaves the destination, and the state goes with them.
The shell therefore owns one `SelectionState` per destination that has tabs and
reads `selected` while rendering, using `Tabs::on_select` only as the redraw
seam — which is exactly the division `SelectionState` documents. A destination
does not keep another's row selection, because a row index means nothing across
collections.

The primary list is `ui::primitives::list`, a new Ordo primitive rather than a
column of Buttons. It adds no dependency: a row wraps the same qualified base
Button every other control does, so it is focused, activated by Enter/Space, and
named like anything else, and everything visible is Ordo's — a title, a detail
line, the badges that classify it, and one shared bottom edge, which is what
`DESIGN.md` asks for before a card and what keeps adjacent rows from doubling a
border between every pair. A row's title and detail both truncate, so the row
carries its accessible name as a tooltip too, and `ui::primitives::truncated`
does the same for the workspace, viewer, and editor titles, which truncate
without a control of their own. It is attached unconditionally: measuring whether
a given string actually clipped costs a frame and buys nothing, since the tooltip
only repeats what is already there. That closes the pointer half of `DESIGN.md`'s
truncation rule and only the pointer half — the pin suppresses a tooltip under
keyboard modality, which is recorded under **Pinned platform limitations** below
— so a keyboard operator reads the full text through the accessible name instead. A `Badge` is a short classifier beside that text;
the liturgical colors reach it through `tone`, which is the one thing
`DESIGN.md` allows them to say. What a row is *not* is a collection: arrow-key
roving and ListBox/option semantics need the collection itself, and land with
the first list that owns real records. The gap is marked in the source and
nothing claims otherwise in the meantime.

A row also declares `flex_none`, and that is not cosmetic. Without it a row is
an ordinary flex child of its column, so a short window squeezes every row down
to its minimum instead of letting the list overflow — the titles close up, and
the scroll indicator has nothing to report because the list was never allowed to
be too long. A regression test shortens the window and asserts both halves: the
row keeps the height it had, and the indicator appears. The same rule holds the
workspace header, which `DESIGN.md` puts among the things that are never
truncated; the panes below it scroll instead, through `ui::scroll` like every
other bounded surface.

The active marker is out of flow, on the rail's leading edge, because the two
rail states want opposite things from it. Expanded, the icons and labels line up
in a column beside it. Collapsed there is no label to line anything up with, and
an icon pushed off-centre by a four-pixel bar reads as crooked, so the icon is
centred in the rail and the marker overlays its edge. A marker in flow would do
the first and prevent the second, and it also charged the collapsed rail width
it no longer costs. The matrix test asserts the centring at every scale where
the rail collapses.

The rail is sized by its own content rather than by a constant. Atkinson is
wide, and a fixed width is a guess that only ever fails one way: a destination
whose label is painted past the edge it was told to fit inside, visible only
when a hover fill showed how far it ran. `rail::EXPANDED_RESERVE` remains as
what the collapse decision reserves for the rail, and the matrix test pins it to
what the rail actually measures — every destination has to end inside the rail
at both viewports and all five scales. Rail items also use the new
`Button::stretch`, which fills the container and puts the icon and label on the
leading edge: a destination is read down a column with six others, and a centred
label reads as ragged.

Opening an entity replaces the whole screen, rail included, rather than swapping
one pane. Its body keeps its own `Scroll` rather than the viewer's: the two are
never on screen together, so one handle would hand the operator back a viewer
scrolled to a position they never chose. The operator is working on one thing,
and Back returns them to the screen they left — the destination, its local view, its row selection, and the
keyboard, all of which the test asserts are gone while the editor is open and
back afterwards.

Regions are `Global navigation`, `Workspace actions`, `Primary list`, and `Main
editor`, in that order. The Primary list is genuinely absent for a
single-surface destination — Settings, Help, Diagnostics — and while a focused
editor owns the workspace, so the shell exercises the same hidden-region
skipping the lab does, on real content. The overlay host is mounted as a
sibling of the registry root, as T0.3.10 requires, and the header's Inspector
opens the labeled right slideover through it. The congregation window is
untouched and still renders neither the registry nor the overlay/status host.

Automated evidence in `ordo::shell`:

- `the_rail_reaches_every_destination_and_tabs_reach_every_local_view`: every
  one of the seven destinations opened by pointer, every local view of the two
  destinations that have them, exactly one active marker at a time, the Primary
  list present only where the destination has one, Settings selecting an
  Interface scale that `Ctrl+0` then resets, and a destination keeping its own
  local view across a round trip through another.
- `the_rail_collapses_and_the_workspace_never_runs_past_the_window`: both
  qualified viewports at all five scales, asserting the rail's measured width
  against its reserve, every destination ending inside the rail, the list
  reporting its position when it overflows and nothing when it fits, and the
  rail, the workspace, and the workspace title all ending inside the window —
  the whole-workspace horizontal scroll `DESIGN.md` forbids, measured rather
  than assumed.
- `a_focused_editor_returns_the_destination_local_view_selection_and_invoker`:
  destination, local view, and row selection surviving the round trip, the list
  region, the tab strip, and the rail all absent while the editor owns the
  screen and all back afterwards, a row keeping its height when the window
  shortens, and the keyboard back on the invoker.
- `the_rail_gives_up_its_labels_before_the_workspace_gives_up_a_pane` and
  `every_destination_is_reachable_and_local_views_belong_only_to_a_destination`
  pin the collapse threshold and the hybrid hierarchy without a window.
- The environment, shortcut, Presenter, and startup-restore tests move from
  `ui` unchanged apart from their view.

`ui-lab` gains a `list` scenario for the new primitive and takes it through the
existing appearance/scale/viewport matrix, asserting that a row's inline content
stays inside its column at every combination.

The local-view strip is a seam across the workspace rather than a row of loose
controls: `Tabs` spaces nothing, and the strip carries no gutter, so the first
view starts on the workspace edge. Views that outrun the strip scroll across it
instead of wrapping, because a wrapped strip moves every tab under it the moment
one more is added or the window narrows. That makes it a bounded scrolling
surface like any other, so `ui::scroll::Scroll` gained an axis: the measurement,
the wheel direction, and the edge the indicator sits on are all read down the
scrolled axis, and `Scroll::horizontal` returns the same handle read across.
`a_strip_too_narrow_for_its_local_views_scrolls_across` pins one line, the
overflow, and the indicator; the responsive matrix pins the flush leading edge
and the tabs meeting end to end at every viewport and scale.

The seam between what a workspace is and what it is showing belongs to the
header, not to the tab strip: the header draws its own bottom edge, and the
strip — where there is one — draws only the edge above itself. The tabs still
sit between a pair of rules, no surface doubles another's border, and a
destination with no local views is ruled like every other one. The navigation
test asserts the header is present at every destination while only the strip
follows the local views.

The rail's active marker is the row's own leading edge in both widths. `MILESTONES.md` asks the active
destination to be non-color-only, and `Button::current` is the assistive half of
that rather than the visible one, so a collapsed rail that dropped the bar would
leave the tint alone for the operator the requirement exists for. What changes
between the two widths is the flow, not the marker: expanded, every row reserves
the same bar, so the icons stay aligned and the row keeps the rail's inset on
both sides; collapsed, nothing is reserved, so the single icon stays centred in
the rail while the state stays visible. What the marker is took three attempts,
and the pin decided each one. It cannot be clipped to the row's corner, because
GPUI's content mask is a rectangle: `overflow_hidden` bounds a child without
following the radius, so a square bar ran out past the curve at both ends. It
cannot carry that corner as its own radius either, because `Style::paint` clamps
a corner radius to half the quad's smaller side, so a four-pixel bar asking for
the four-pixel control radius is given two and comes out visibly tighter than the
control beside it. What works is the left border of a quad the size of the whole
row: the radius is unclamped there, so the marker's curve is literally the row's
corner. That quad is out of flow and inert — a div with nothing but a border
inserts no hitbox at this pin — so it neither moves the icon nor takes the
pointer, and being the row's last child it is drawn over the control's hover
fill rather than under it, which an icon-only Button filling the whole row would
otherwise cover the moment the pointer arrived. The matrix pins the minimum quad
size the clamp makes load-bearing, and
`the_active_rails_marker_does_not_take_the_pointer` pins the inertness, because
nothing else can: every other rail click in these tests lands on a row that is
not the active one, and clicking the active one asserts nothing on its own,
since opening the destination it is already on is a no-op either way. The test
fails as soon as the quad is given a hitbox.

What the row does *not* do is clip. GPUI masks a child's paint to the parent's
rectangle through `Style::overflow_mask`, and that mask covers box shadows —
which is what Ordo's focus treatment is made of, two outset rings. An
`overflow_hidden` here therefore erased the focus ring from every rail
destination, in the first region the `F6` cycle reaches, and did it in both
widths: collapsed, an icon-only Button fills the row and every side of its ring
fell outside; expanded, only the segment lying in the marker's own band
survived. Nothing needs the clip once the marker carries its own corner, since
the Button already rounds its fill to the same radius, so the row does not set
it. The expanded reserve is the bar plus `FOCUS_SEPARATION + FOCUS_OUTER` for
the same reason: with only the bar reserved, a focused active destination puts
the ring and the marker in one four-pixel band, and `DESIGN.md` has focus and
selection coexisting rather than taking turns. Collapsed nothing is reserved and
the rings fall outside the row entirely, which the rail's own inset leaves room
for. The responsive matrix asserts the
marker's position at every combination, the icon's centring where the rail
collapses, and that every row measures the same: a row whose control is sized by
its own label leaves seven fills at seven widths staggered down the rail, so the
control takes what the marker leaves rather than what its label needs. The
Settings panel marks its current appearance and scale the same way, with a
`CHECK` glyph as the shape cue beside the same `Button::current` semantics — and
it reserves that glyph's slot on every choice for the rail's own reason, because
the choices sit on a wrapping flex line and one that grew when it was taken would
shove its neighbours along and drop the last to the next row. The navigation test
takes an appearance and asserts the other choices did not move.

`list::arrows` binds Up and Down to the window's focus order from the list
container, because a column of records is one collection the operator reads
through rather than a run of controls only `Tab` connects. It rides the tab
order rather than roving focus, so it steps out of the list at either end; that
goes with the ListBox semantics `ListRow` already owes.
`the_arrows_move_between_list_rows` compares the focus handle the arrows reach
against the one clicking that row produces, which is the only assertion that
says *which* row was reached.

A badge is a solid fill in its liturgical color with a label picked for contrast
against it, rather than an outline: an outline is only as visible as the domain
color happens to be against the row, and liturgical white on a Preparation
surface was very nearly nothing. `DESIGN.md`'s liturgical colors are normative
and the same five in both appearances — they name a season, not a theme — so the
badge adapts by choosing what it writes on them.
`every_liturgical_badge_is_legible_in_both_appearances` pins every season and the
plain badge at WCAG AA in Light and Dark.

Validation: `cargo xtask verify` passed all nine gates on 2026-09-06, including
83 UI tests, 14 shell/runtime tests in `ordo`, the native runtime harness,
Clippy, release configuration, and cargo-deny. This evidence does not close all
of T0.3; the E2E path, the manual keyboard-only pass, and the Windows High
Contrast check remain with T0.3.12.

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

A truncated value's keyboard reveal is a fourth. `DESIGN.md` requires every
visually truncated value to show its full text "on pointer hover **and keyboard
focus**", and the pin supplies only the pointer half: `div.rs` registers a
tooltip through `register_tooltip_mouse_handlers` and nothing else, and there is
no focus trigger anywhere in the pinned GPUI. It does not merely omit the other
half — it suppresses it. `Hitbox::is_hovered` returns `false` outright when
`Window::last_input_was_keyboard`, and the prepaint hover check a tooltip is
gated on begins with the same negation, so tabbing to a truncated control shows
nothing. Ordo therefore closes the pointer half with `ui::primitives::truncated`
and the `ListRow`'s own tooltip, and the full text reaches a keyboard operator
through the accessible name, which every truncated value already carries. A
revision that triggers a tooltip from focus, or that stops suppressing one under
keyboard modality, closes the visible half and reruns this qualification.

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
