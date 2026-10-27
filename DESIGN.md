---
version: alpha
name: Ordo
description: A quiet, dependable visual system for Catholic liturgical preparation and live presentation.
colors:
  preparation-background: "#F4F0E7"
  preparation-surface: "#FFFDF8"
  preparation-surface-muted: "#EBE5DB"
  preparation-text: "#27231F"
  preparation-text-muted: "#716B62"
  preparation-border: "#D7CFC2"
  preparation-border-strong: "#8E8476"
  presenter-background: "#151816"
  presenter-surface: "#222722"
  presenter-surface-raised: "#303630"
  presenter-text: "#F2EFE8"
  presenter-text-muted: "#B1ADA4"
  presenter-border: "#414842"
  presenter-border-strong: "#7A857B"
  primary: "#355D4B"
  on-primary: "#FFFFFF"
  selection-preparation: "#DFE9E2"
  selection-presenter: "#26362C"
  live-preparation: "#2F6B4F"
  live-presenter: "#55D69C"
  freeze-preparation: "#9A5B10"
  freeze-presenter: "#F0B35E"
  blackout: "#050606"
  on-blackout: "#FFFFFF"
  warning-preparation: "#9A5B10"
  warning-presenter: "#F0B35E"
  danger-preparation: "#9D3E31"
  danger-presenter: "#EF8E82"
  information-preparation: "#315E83"
  information-presenter: "#9ACDF2"
  focus-preparation: "#2F6FA3"
  focus-presenter: "#8FCBFF"
  projection-background: "#000000"
  projection-text: "#FFFFFF"
  liturgical-purple: "#6D4A87"
  liturgical-red: "#A43F38"
  liturgical-green: "#46744D"
  liturgical-rose: "#C9788E"
  liturgical-white: "#F9F5E9"
rounded:
  control: 4px
  overlay: 8px
  slide-viewport: 4px
shadows:
  floating-preparation: "0 4px 12px rgba(39, 35, 31, 0.14)"
  overlay-preparation: "0 18px 48px rgba(39, 35, 31, 0.20)"
  floating-presenter: "0 4px 14px rgba(0, 0, 0, 0.40)"
  overlay-presenter: "0 18px 52px rgba(0, 0, 0, 0.58)"
iconography:
  family: Lucide
  control-size: 16px
  navigation-size: 20px
  stroke-width: 2px
spacing:
  unit: 4px
  related-tight: 4px
  related: 8px
  control-inset: 12px
  group: 16px
  section: 24px
density:
  compact-control: 32px
  default-control: 36px
  presenter-control: 40px
  workspace-gutter-compact: 16px
  workspace-gutter-wide: 24px
motion:
  quick-duration: 120ms
  standard-duration: 160ms
  overlay-open-duration: 400ms
  overlay-close-duration: 200ms
  output-content-duration: 0ms
  output-blank-duration: 500ms
  easing: "cubic-bezier(0.2, 0, 0, 1)"
typography:
  operator-title:
    fontFamily: Atkinson Hyperlegible Next
    fontSize: 24px
    fontWeight: 600
    lineHeight: 1.2
  operator-body:
    fontFamily: Atkinson Hyperlegible Next
    fontSize: 16px
    fontWeight: 400
    lineHeight: 1.45
  operator-label:
    fontFamily: Atkinson Hyperlegible Next
    fontSize: 14px
    fontWeight: 500
    lineHeight: 1.3
  projected-response:
    fontFamily: Atkinson Hyperlegible Next
    fontSize: 113px
    fontWeight: 800
    lineHeight: 1.15
---

# Ordo Design System

`DESIGN.md` is the canonical visual-system contract for Ordo. Its YAML tokens
are normative; this prose explains why the tokens exist and how to apply them.
Product behavior remains in `MVP-PLAN.md`, historical rationale remains in
`docs/decisions/`, and domain vocabulary remains in `CONTEXT.md`.

## Overview

Ordo should feel quiet, capable, and trustworthy under pressure. Preparation
work resembles a well-kept working document: warm, dense, legible, and calm.
Presenter Mode becomes graphite-dark so the operator can concentrate on the
canonical Preview and Program frames in a dim church.

The interface is modern without depending on fashionable dashboard patterns.
Hierarchy comes from typography, alignment, restrained tonal contrast, and
clear ownership of actions. It should never resemble generic AI-generated
software assembled from nested cards, oversized headings, decorative gradients,
or indiscriminate pills.

This document primarily governs Ordo's operator interface. Congregation-facing
Slide Themes are parish content and remain independently configurable. Canonical
slide previews must nevertheless preserve the adopted `16:9` geometry and use
the same renderer as Program and congregation output.

## Colors

Preparation uses warm paper neutrals for prolonged editing. Presenter Mode uses
neutral graphite rather than a tinted branded dark mode. Both appearances share
one semantic state language:

- **Primary and selection:** restrained forest green.
- **Live:** green, with the brighter value reserved for the graphite Presenter
  appearance.
- **Freeze and warning/incomplete:** amber.
- **Blackout:** near-black with a light label and border.
- **Error and destructive actions:** muted red.
- **Information:** restrained blue.

Operational state is never conveyed by color alone. Every Live, Freeze,
Blackout, warning, unavailable/error, and informational treatment also has an
explicit label and a stable icon or shape cue.

Liturgical purple, red, green, rose, and white are domain colors. Use them only
when representing actual liturgical metadata or congregation-facing Theme
content. They are not interchangeable application accents, selection colors,
or operator-state colors.

Token values may be adjusted only through an explicit design-system change and
must retain their semantic role and pass the adopted accessibility contrast
requirements.

Preparation exposes System, Light, and Dark appearance. System is the default
and follows operating-system appearance changes while Ordo is running.
Presenter Mode always uses graphite dark; leaving Presenter restores the saved
Preparation preference. Operator appearance and congregation Theme remain
independent inputs.

## Typography

Bundle `Atkinson Hyperlegible Next` locally and use it throughout operator and
congregation surfaces. Use Regular for ordinary reading, Medium and SemiBold for
operator hierarchy, and reserve ExtraBold primarily for projected lyrics and
congregation responses.

Bundle `Noto Sans` as the missing-glyph fallback. Never depend on fonts installed
on the host operating system, and never fetch fonts at runtime.

Atkinson is comparatively wide. Preserve readable sizes and adapt the layout,
truncate deliberately, or disclose secondary information rather than shrinking
important labels to fit. Exact operator type levels remain subject to visual
qualification at both required desktop resolutions.

## Iconography

Use bundled Lucide SVG icons as Ordo's sole general operator-interface family.
Use `16px` icons inside ordinary controls and `20px` icons for navigation and
prominent actions, with a consistent `2px` stroke. Import only the icons used by
the application, retain their license notices in distributed third-party
notices, and never require a network icon source at runtime.

Pair icons with visible text for high-stakes or unfamiliar actions, including
Live, Freeze, Blackout, Present, Restore, Delete, and congregation-display
control. Limit icon-only controls to familiar compact actions such as Close,
More, disclosure chevrons, and drag handles; every icon-only action has an
accessible name and a pointer tooltip that repeats that name.

An icon that describes its action leads the label. An icon that says a surface
opens from this control — a Select or Menu chevron — trails it, pointing where
that surface appears. A disclosure chevron is the exception: it maps to nesting
rather than to a popup, so it keeps the leading edge beside the indent.

Ordinary icons remain outlined. Filled shapes are reserved for status markers
whose shape is part of the non-color state cue. A custom SVG is permitted only
when an Ordo-specific concept cannot be expressed clearly with Lucide; it must
use the same visual grid, stroke language, sizing, and accessibility rules.
Never mix Lucide with Material, Font Awesome, emoji, or another general icon
family inside the product interface.

## Layout

Preparation uses a persistent application rail and focused workspaces rather
than a dashboard landing page. Dense editors may use master-detail or three-pane
layouts when each pane has stable ownership. Canonical slide previews retain
their `16:9` aspect ratio and composition regardless of available pane width.

Presenter Mode has a slim global header, an optional Ordo rail, a grouped Cue
timeline, and a stable right-hand Output Dock. Program, Preview, output state,
Freeze, Blackout, and the context-sensitive primary output action must remain
visible and spatially stable.

Dialogs and slide-overs are bounded to the visible application viewport,
including the application header. Preserve exterior gutters. If content exceeds
the viewport, keep the overlay header and footer actions reachable while only
the body scrolls. Any bounded surface that scrolls shows a thin overlay scroll
indicator while its content exceeds its viewport, using the strong border token
over the scrolled content rather than reserving a gutter. It is an indication of
position, never the only way to scroll, and it never appears on a surface whose
content fits. It may also be dragged: it stays thin while it is only
reporting a position and widens to a grabbable width when the pointer
reaches for the edge, so it never covers more of the content than the
moment requires, and it stops clear of the surface's own corner radius at
both ends rather than riding out over it. Wheel and keyboard scrolling
are unchanged by it. A bounded scrolling surface also contains the wheel it
handles: pointing at a popup list inside a slideover body scrolls that list
alone, never the body behind it or the workspace behind that. A nested
slide-over advances from the right while its parent recedes slightly and
returns without losing parent state.

Responsive layouts are qualified at `1366x768` and `1920x1080`. Resizing may
change information density, column count, or disclosure, but it must never
change canonical slide composition, semantic state, or command ownership.

Use the `4px` spacing unit and its adopted scale rather than arbitrary local
gaps. Keep closely related controls `8px` apart, groups `16px` apart, and major
sections `24px` apart; use `4px` and `12px` only where their named roles fit.
At `1366x768`, use `16px` workspace gutters. At `1920x1080`, increase
major workspace gutters to `24px` while giving most additional area to wider
previews, more slide columns, and useful context.

Default controls and ordinary list rows are `36px` high. Compact icon controls
may be `32px` square, while high-stakes Presenter controls are at least `40px`
high. Do not proportionally enlarge the whole interface as resolution rises,
and do not add an MVP Compact/Comfortable preference. Increased interface text
scaling may expand rows and reflow secondary content; labels and controls must
not clip or overlap to preserve a fixed density.

## Canonical Slide Layouts

Compose congregation Slides in renderer-owned semantic header, body, and footer
zones on a `1920x1080` logical canvas. Use `120px` left/right and `80px`
top/bottom as the minimum default safe area. A Theme may increase those insets
but may not reduce them. Header and footer occupy only their measured content
plus the adopted gap; when either is absent, the body receives that space.
Never expose freely draggable positioning in the MVP.

Use these fixed template relationships:

- **Lyrics:** optional repeated section label at top-left; authored-case body
  optically centered in the remaining region, each line centered
  horizontally.
- **Response:** optional repeated ExtraBold authored header resolved before
  rendering (for example, the Cebuano template default `TUBAG:`); stored
  authored text in authored case as the emphasized body at weight `800`,
  sized to content: the largest of the `160`/`140`/`120`/
  `100px` sizes whose block fits the body zone, with `100px` as the floor
  that carries any overflow warning; centered horizontally and vertically in
  the available body zone.
- **Reading identification:** proclamation title preserved exactly as authored
  (mixed case, never uppercased) at `135px`/weight `800`,
  left-aligned to the safe-area left edge and centered vertically in the body
  region above the unchanged bottom-left Scripture reference. Do not add
  First Reading, Second Reading, or Gospel labels to congregation output.
- **Reading passage:** authored-case ordinary-weight Scripture at
  `88px`/weight `400` (ReadingPassage only, `70 * 1.25 = 87.5`
  rounded), left-aligned to the safe-area left edge and top-anchored, smaller
  than identification, with the unchanged `50px` reference at bottom-left on
  every Slide.
- **Responsorial Psalm:** optional small ExtraBold authored header resolved
  before rendering (for example, the Cebuano template default `SALMO
  RESPONSORYO`) at top-left, emphasized refrain at weight `700` centered in
  the remainder, sized to content from the `140`/`120`/
  `105`/`90px` sizes (floor `90px`), and reference at bottom-left. A projected cantor verse uses Reading-passage
  treatment.
- **Song identification:** a bottom-left identity block with an authored-case
  Song title at `100px`/weight `700`, smaller composer/source, and optional Arrangement/Choir credit.
- **Celebration title:** title preserved exactly as authored (mixed case,
  never uppercased) at `135px`/weight `800`, left-aligned to the
  safe-area left edge, plus an optional Normal-Case subtitle at
  `68px`/weight `300` on the same left edge below the title; title plus
  subtitle center together vertically above the unchanged bottom-left
  localized date. The subtitle reuses the header slot renderer-locally with
  no new product field: absent, empty, or whitespace-only reserves no
  vertical space.
- **General Text:** optional header, center-left body at `70px`, and optional footer.
  GeneralText stays at `70px` and is untouched by the ReadingPassage-only `88px`
  candidacy.
  **General Image:** one contained foreground image plus optional header and
  caption. A Theme image is background, not foreground content.

Preserve authored line breaks. A blank source line is a hard Slide boundary for
Lyrics and Scripture and overrides automatic proposals. Pagination targets two
logical lyric phrases and no more than four rendered lines; split before
shrinking, never clip, and emit a readiness warning when content cannot fit at
the approved minimum. Limit Song titles to three rendered lines and Celebration
titles to three rendered title lines (subtitle lines never count) before
warning. Validate measured canonical geometry rather than character
counts.

The relative hierarchy and the projected levels are approved at the T0.4
manual gate (Decision 267): display `90px` / passage `70px` / meta `50px`
(each exactly +25% from the PR 5 provisional `72px` / `56px` / `40px`), plus
the shared Celebration/Reading-identification title at `135px`/weight `800`
(50% above display) with a Celebration-only optional Normal-Case subtitle at
`68px`/weight `300` (75% of display), plus a ReadingPassage-only `88px`/weight
`400` (`70 * 1.25 = 87.5`, rounded; line advance `102px`), left-aligned and
top-anchored. GeneralText stays at `70px`. Psalm and Response bodies are
content-sized from the ladders above (Psalm `140`–`90px`/weight `700`,
Response `160`–`100px`/weight `800`), and Song titles lead at `100px`/weight
`700`. `PROVISIONAL_DISPLAY_SIZE` stays at `90px` for Lyrics and the Psalm
floor (the `PROVISIONAL_*` constant names are historical). Operator, UI, and
lab-control typography are unchanged except the `projected-response` token,
`113px`/weight `800` for lab specimens while congregation Response output
follows its ladder. The long Cebuano and English cases were judged
comfortably readable at these levels; the approved captures are
`design-references/ordo-canonical-slide-layouts/`.

## Elevation & Depth

Prefer tonal layers, separators, and borders over floating-card shadows.
Ordinary content should feel part of one workspace rather than a field of
detached cards.

Use `1px` neutral separators as the ordinary structural treatment. The regular
border tokens divide adjacent panes, rows, sections, and other content whose
relationship is already apparent. Prefer a single shared edge or a restrained
surface change over wrapping each item in its own bordered container, and never
allow adjacent surfaces to produce doubled borders.

Use the strong border tokens only where the edge itself must remain clearly
perceptible: input or control boundaries that lack another affordance, canonical
frame boundaries, active drop targets, and temporary overlay edges. Do not use
strong borders as decoration or to box every group. Selection uses its adopted
forest-green tinted fill rather than introducing a competing green border;
keyboard focus remains the independent two-layer blue outline.

Reserve visible elevation for temporary ownership changes: menus, dialogs,
slide-overs, and the focused child in a nested slide-over sequence. Shadows are
restrained and must clarify stacking, not decorate the page. Never use glow as
an elevation or operational-state treatment.

Use exactly two shadow levels per appearance. **Floating** applies to menus,
popovers, dropdowns, and tooltips. **Overlay** applies to dialogs, slide-overs,
and the focused child in a nested slide-over sequence. Both retain their
adopted neutral border so the edge remains legible without depending upon the
shadow alone.

Dialogs, ordinary slideovers, and unpinned Presenter Ordo use the same visible
dim scrim. It captures pointer input entirely: clicking the scrim dismisses the
temporary surface without activating anything beneath it, and a control the
scrim covers does not respond to the pointer at all — no hover treatment, no
pointer cursor, no tooltip. Pinning Ordo removes the scrim
and converts it to a fixed pane. Every overlay and its scrim share one
application-owned stacking layer so background controls cannot receive stray
events.

Fixed panes, list rows, buttons, selected items, and canonical slide frames have
no shadow. Nested slide-overs use one overlay shadow plus the adopted parent
recession; they do not accumulate increasingly dramatic shadows with nesting.
Presenter shadows use neutral black suited to graphite surfaces. Never color a
shadow to imply Live, Freeze, Blackout, warning, focus, selection, or liturgical
state, and never use a glow as a substitute.

## Shapes

Use the restrained two-level radius system consistently. Buttons, inputs, menu
items, interactive-row treatments, compact controls, and slide-viewport chrome
use the `4px` control radius. Menus, dialogs, slide-overs, and other temporary
elevated surfaces use the `8px` overlay radius. Fixed panes, shared dividers,
tables, and ordinary open list structures remain square.

Every operator-facing slide viewport—including Cue thumbnails, generated-frame
navigators, Inspector previews, Preview, and Program—clips its canonical render
inside the `4px` slide-viewport chrome. Apply the radius identically to Preview
and Program. The renderer still produces a square-cornered `1920x1080` canvas,
and congregation output presents that canvas square and edge-to-edge on the
physical display. Rounded operator chrome is not part of a Slide Theme, saved
slide content, or exported output.

The adopted safe-text margin keeps meaningful text away from the small masked
corners, but the radius may not change content layout, scaling, pagination, or
the canonical `16:9` geometry. The keyboard-focus outline follows the viewport
wrapper rather than entering the rendered content.

Reserve fully rounded forms for genuine circles, status dots, and compact status
labels whose semantics warrant the silhouette. Do not turn ordinary buttons,
containers, or list rows into pills, and do not mix unrelated radius values in
the same workspace.

## Components

Ordo owns its visible product vocabulary. The internal native `ui` primitive Module
is implemented in GPUI, consumes these tokens, and must not make the product
resemble a stock component-library demo. A third-party GPUI behavior primitive
may sit behind that Module only after qualification; it never defines Ordo's
visual language.

- Use one clear primary action per local decision surface.
- Keep secondary and quiet actions visually subordinate without making them
  undiscoverable.
- Keep destructive actions red, explicitly worded, and separated from routine
  navigation.
- Use bordered rows and disclosures for structured information before reaching
  for cards.
- Use a **Select** — a button trigger plus a popup list — to choose from a
  fixed set the operator can read. Use a **Combobox** — an editable field
  trigger plus a filtered popup list — only where typing is genuinely useful:
  a long or open set, or one that accepts a name outside it. Both collapse to
  their current value; neither is a substitute for the other, and a Select
  never shows a caret.
- Keep high-stakes Presenter controls labeled and stationary.
- Place operator-only toasts at bottom-center. Use transient toasts for
  confirmations and Undo; persistent failures and hazards remain visibly owned
  by the affected surface until resolved or dismissed deliberately.
- Render canonical thumbnails, Preview, Program, and congregation output from
  the same slide composition.
- Use Disabled only for a control that cannot currently execute, Unavailable for
  content or capability that cannot fulfill its purpose, Read-only for an
  intentionally inspectable non-editing context, and Not assigned for a valid
  empty configuration.

Component implementations consume these shared tokens through the Ordo-owned
design-system Module. Do not introduce a parallel local radius, shadow, spacing,
focus, icon, or motion vocabulary inside an individual workspace.

## Do's and Don'ts

- Do make hierarchy legible through alignment, typography, and restrained tonal
  contrast.
- Do pair color with text and icon/shape cues.
- Do preserve operator context beneath temporary overlays.
- Do keep congregation content visually isolated from operator chrome.
- Do keep the interface usable with keyboard, pointer, and increased text size.
- Do use the approved design references to understand composition and behavior.
- Don't create generic card mosaics or dashboard homepages.
- Don't use decorative gradients, glows, glass effects, or arbitrary accent
  colors.
- Don't use pills for ordinary containers, list rows, or every label.
- Don't let dialogs or slide-overs grow beyond the visible application viewport.
- Don't recolor the operator shell using a Celebration's liturgical color or a
  congregation Theme.
- Don't silently move, hide, or restyle high-stakes Presenter actions in ways
  that change their operational meaning.

## Interaction States

Disabled controls use neutral, subdued styling without warning or danger color.
Use native disabling when the reason is obvious. A consequentially blocked
action remains keyboard-focusable, exposes its unavailable state through
AccessKit, suppresses activation, and references a persistent nearby
explanation. A hover-only tooltip is never the sole explanation.

Unavailable content remains visible and legible. Pair the word **Unavailable**
with an icon and short reason; retain inspection and corrective navigation while
blocking any action that would expose or commit invalid content. Do not make an
Unavailable Cue look like an inert disabled button.

In a Read-only context, omit editing controls where practical and state why the
content is inspectable but not editable. Do not approximate Read-only by fading
an entire editable form. **Not assigned** is a valid choice and uses ordinary
neutral treatment; it is distinct from a previously assigned resource becoming
Unavailable.

Show the custom keyboard-focus treatment only when GPUI's focus/input modality
indicates keyboard navigation, not indiscriminately after pointer activation.
Use a two-layer treatment: a `2px` separation ring matching the
immediate surface, followed by a `2px` outer ring using `focus-preparation` or
`focus-presenter`. The separation is required so focus remains distinct on
primary, semantic-state, destructive, and informational controls.

Focus and selection are independent and may coexist. Selection remains a
forest-green tinted fill; focus remains the blue outer outline around the
currently keyboard-active control or row. Put the focus treatment on the outer
wrapper of canonical slide thumbnails and frames so it never changes or covers
the inner `16:9` composition. Do not reuse Live, Freeze, Blackout, warning,
danger, selection, or liturgical colors as focus indicators.

Never suppress operating-system focus indication unless the complete Ordo focus
treatment replaces it. In Windows High Contrast or equivalent platform modes,
allow the operating system's highlight colors and outlines to take precedence.
Focus must remain visible when interface text is enlarged and when it appears
inside menus, dialogs, nested slide-overs, the Mass Outline, Cue timeline,
Preview/Program controls, and dense editor lists.

## Focus Navigation

Keep GPUI focus order and visible reading order identical. `Tab` and `Shift+Tab`
advance normally through interactive controls and are not trapped by ordinary
workspace panes. `F6` and `Shift+F6` provide the faster desktop path among
visible major regions: global navigation, workspace header/actions, primary
list or Mass outline, main editor, and Inspector. Skip regions that are absent,
hidden, or collapsed.

Selection may refresh contextual content without moving keyboard focus away
from the selected row. Lists, trees, grids, tab sets, menus, and Cue collections
retain their expected arrow-key behavior and remember a meaningful focus target
when the operator returns to their region. A focused dialog or slide-over owns
focus until closed and then restores it to its invoker or the nearest surviving
relevant entity.

Presenter Mode reduces the region cycle to the visible Ordo rail, Cue timeline,
and Output Dock/controls. Region navigation is operator chrome only: it never
changes Preview, Program, Freeze, Blackout, or congregation-output state.

## Interface Scaling and Slide Isolation

Offer Interface scale at `100%`, `125%`, `150%`, `175%`, and `200%`. Enlarge
operator typography, controls, spacing, hit areas, and focus geometry together,
then let responsive disclosure and reflow absorb the reduced workspace. Store
this as an installation preference. It does not belong to Parish content or a
Slide Theme.

Windows display scaling and Ordo Interface scale apply only to operator
chrome. They may resize a thumbnail viewport or reduce its grid column count,
but never alter the frame inside it. Do not apply a global GPUI/window scale to
the canonical frame for this feature. The Canonical Slide Renderer composes in an explicit `1920x1080`
logical coordinate system independent of GPUI viewport size, operator Interface
scale, operating-system scale, and physical output density.

A higher output density may rerasterize the same completed geometry for
sharpness. Font metrics, wrapping, pagination, safe area, and element positions
remain invariant. Thumbnail, Inspector, Preview, Program, congregation output,
snapshot frames, and emergency export all consume that same composition; their
surrounding viewport can scale, but the Slide cannot reflow.

## Assistive Semantics

Name major regions with the same visible domain language used by operators:
Global navigation, Workspace actions, the applicable primary list or Mass
outline, main editor, Inspector, Cue timeline, Program, Preview, and Output
controls. A Cue's concise accessible name contains its position, meaningful
title or first line, type, and project/wait/hold/Blackout behavior. Associate
complete Slide or private-context text as a description instead of placing it
all inside the control name.

Focused navigation provides ordinary selection feedback; do not echo every
arrow press through a live region. Use one deduplicated polite status channel
for expected operational changes such as Freeze, Blackout, taking Preview live,
and output enablement. Interrupt urgently only for unexpected safety events,
including display disconnection, unavailable Cue, automatic Freeze, or loss of
usable Program output, and state both the event and Ordo's protective response.

Operator-only prompts, timing context, warnings, and diagnostics remain
assistive content only in the operator workspace. Never mirror those semantics
into the passive congregation renderer.

## Constrained Preparation Layouts

At ordinary `1366x768` and `100%` Interface scale, keep the complete primary
list or Mass outline, editor, and Inspector composition. Use minimum logical
pane widths of `240px` for the list/outline, `520px` for the editor, and `300px`
for Inspector. Decide adaptation from the effective workspace remaining after
system scale, Interface scale, window size, navigation chrome, separators, and
those minima—not from physical resolution alone.

When width first becomes constrained, preserve the list/outline beside the
editor and move Inspector into its labeled right slideover. At narrower widths,
keep the editor as the sole fixed surface and expose list/outline and Inspector
as opposing labeled slideovers. Only one temporary side pane opens at a time.
Preserve selection, scroll, editor state, and meaningful focus across every
transition and restore fixed panes when sufficient width returns.

Never introduce whole-workspace horizontal scrolling or compress the editor
beyond useful control and label geometry. Keep the current entity and primary
workspace action visible; lower-priority actions may enter a clearly labeled
overflow without losing stable wording or keyboard access.

## Constrained Presenter Layouts

The wide three-column Presenter remains the preferred operating experience. At
`1920x1080`/`100%`, pin the Ordo rail and keep the Cue timeline beside its
dedicated Output Dock. At `1366x768`/`100%`, begin with the Ordo rail collapsed
and retain timeline plus stacked Program/Preview monitoring.

When effective dimensions can no longer support that composition, make Ordo an
overlay and move output monitoring above the timeline in the main Presenter
column. Program and Preview remain simultaneously visible in one fixed,
side-by-side `16:9` band using identical canonical viewport geometry. Keep
output status, Freeze, Blackout, and the contextual primary action immediately
below it; these controls never enter overflow or scroll away.

The Cue timeline alone consumes and scrolls through the remaining area. Keep
the selected Grid/List mode—even if Grid reaches one column—and move only
low-risk header actions into overflow. Responsive reflow never hides a monitor,
creates whole-Presenter scrolling, or changes Cue selection, Preview, Program,
Freeze, Blackout, or output state. Restore the preferred wide composition when
effective space returns.

## Truncation and Information Priority

Never truncate Live, Freeze, Blackout, congregation-output health, active
warnings/errors, readiness counts, scheduled times, or contextual primary
actions. Reflow them and remove lower-priority context first. A current
workspace/entity title may wrap to two lines; narrow list, outline, and Ordo
rows use one-line ellipsis; Cue captions may use two lines outside the canonical
frame.

Every visually truncated value reveals its full text on pointer hover and
keyboard focus and retains that full accessible text. Collapse older breadcrumb
ancestors into an accessible overflow before shortening the current entity.
Move secondary metadata into its Inspector/detail owner before compressing
primary identity, state, or action wording.

Never ellipsize editable lyrics, readings, responses, announcements, notes, or
other authored prose. Wrap or locally scroll them inside their bounded owner.
Canonical Slide text is governed only by reviewed pagination and the Canonical
Slide Renderer. Do not use marquee, auto-scrolling labels, hover-only essential
meaning, or below-minimum font shrinking to conceal overflow.

## Motion

Motion is functional and restrained. Use `120ms` for hover, pressed, and color
transitions and `160ms` for menus, popovers, disclosures, and visible
reordering. Dialogs and slideovers enter in `400ms` with fast-start/slow-finish
motion and exit in `200ms` with smooth acceleration; nested parent recession
follows the same direction. Do not use spring, bounce, overshoot, decorative
looping, or attention-seeking animation.

All motion remains interruptible and may never delay accepting the next input.
Content-to-content Preview and Program changes remain immediate cuts. Content
to Blackout/Hold and Blackout/Hold to content use a `500ms` opacity fade,
independently for Preview and Program. A contrary command reverses from current
opacity with proportional remaining duration; presentation commands never
queue. Operator state changes immediately even while the output fade completes.

Under `prefers-reduced-motion`, remove translation, scaling, animated
reordering, and parent recession. A short opacity transition of at most `100ms`
may remain where it clarifies appearance, including blank/content boundaries.
Focus order, surface ownership, state transitions, and final layouts remain
identical.

## Approved References

The interactive references in `design-references/` demonstrate approved
information hierarchy and interaction behavior. They are not production source
and may contain explicitly documented superseded details.

- `design-references/ordo-semantic-palette*` is the current semantic-color and
  appearance reference.
- `design-references/ordo-presenter-three-column*` and
  `design-references/ordo-timeline-modes*` establish Presenter composition.
- `design-references/ordo-presenter-high-scale*` establishes the approved
  constrained/high-scale Presenter hierarchy and the isolation of canonical
  frames from operator Interface scaling.
- `design-references/ordo-mass-planning-responsive*` establishes the approved
  three-pane Mass Planning hierarchy and density at `1366x768` and `1920x1080`
  while holding selected content and canonical Slide composition constant.
- `design-references/ordo-canonical-slide-layouts/` is the approved
  fixture-backed Canonical Slide Layout Gallery: every canonical template,
  label/background variants, and representative overflow warnings at
  `1920x1080`, with per-fixture provenance in `capture-rows.md`.
- Song, Arrangement, import, History, pagination, readiness, and overlay
  references are indexed in `design-references/README.md`.

Authority is by subject: this file owns visible behavior and visual tokens;
`CONTEXT.md` owns domain terms and invariants; `MVP-PLAN.md` owns product scope
and strategic architecture; and `MILESTONES.md` owns task status and acceptance
gates. The records in `docs/decisions/` supply adopted detail only where these shorter contracts
are silent. Approved references illustrate the contract and yield to it; any
conflict between normative documents must be reconciled rather than resolved by
silently choosing one. Update this file whenever an adopted visual rule or token
changes.
