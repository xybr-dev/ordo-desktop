# Ordo design references

These files preserve the interactive design explorations created during the
MVP interview. They are interaction and information-hierarchy references, not
production application code. They yield to the applicable normative owner:
`CONTEXT.md` for domain language and invariants, `DESIGN.md` for visible
behavior and tokens, `MVP-PLAN.md` for product scope and strategic architecture,
and `MILESTONES.md` for implementation status and acceptance gates.

Each exploration has two forms:

- `*-standalone.html` opens directly in a browser.
- The matching non-standalone file is the editable HTML fragment from which
  the standalone version was rendered.

## Explorations

- `ordo-presenter-three-column*` — the three-column Presenter workspace:
  Ordo titles, grouped cue timeline, and Preview/Program state.
- `ordo-presenter-high-scale*` — the approved responsive Presenter acceptance
  reference across `100%`–`200%` Interface scale, including the constrained
  main-column Program/Preview band, fixed live-output controls, retained
  Grid/List choice, and overlay-only Ordo rail.
- `ordo-mass-planning-responsive*` — the approved paired Mass Planning
  acceptance reference at `1366x768` and `1920x1080`, preserving one selected
  Song and canonical Cue while demonstrating compact three-pane density versus
  wider visible context and Inspector preview area.
- `ordo-timeline-modes*` — list and responsive grid treatments for the cue
  timeline.
- `ordo-hybrid-ux*` — reusable song sections, choir arrangement authoring, and
  frozen Preview selection.
- `ordo-arrangement-pagination*` — encode-once song sections and deterministic
  slide pagination.
- `ordo-arrangement-details*` — the adaptive full-app Arrangement-details
  slideover for staged creation and shared-record editing.
- `ordo-song-library-master-detail*` — persistent Song search/results on the
  left and selected Song identity plus Arrangements on the right.
- `ordo-song-import-review*` — automatic-first Song JSON classification,
  evidence review, read-only imported-versus-library comparison, and atomic
  commit staging.
- `ordo-history-revision*` — grouped Song/Arrangement History, nested
  current-versus-revision comparison, guarded whole-entity restoration, and
  immediate Undo feedback.
- `ordo-inspector-cue-navigation*` — selected-item Cue navigation synchronized
  between the editor and Inspector, including black congregation output plus
  private context for operator-only triggers and explicit item boundaries.
- `ordo-incomplete-preflight*` — centered, viewport-bounded acknowledgment for
  incomplete Presenter entry, with compact unavailable-Cue findings, a
  recommended route back to Readiness, and explicit presentation of available
  content without a checkbox ritual.
- `ordo-semantic-palette*` — the adopted warm-light and graphite-dark semantic
  palette, including interactive Live, Freeze, Blackout, warning, unavailable,
  informational, and separate liturgical-color treatments.
- `ordo-song-merge-entry*` — post-MVP research for full-app Song-merge entry,
  duplicate discovery and preview, explicit survivor/retired roles, and
  interactive survivor swapping.
- `ordo-song-merge-review-flow*` — post-MVP research for the approved two-step
  Song-merge workflow in one parent slideover, with preserved Back navigation
  and a nested Arrangement comparison that temporarily recesses the parent
  review.

When production UI work begins, preserve the workflows and information
hierarchy demonstrated here, but reimplement them as native GPUI views using
Ordo's finalized visual language. The HTML and CSS are acceptance references,
not production source or a component-migration contract.

## Known superseded details

- Presenter starts with congregation output disabled, not Live. Enabling the
  output creates a black Program frame.
- While Live/unfrozen, Cue navigation updates Preview and Program together.
  The older permanent `Take selected` treatment is not the final state model.
- Different Arrangements own independent section text, sequence, and
  pagination; edits do not propagate between choir Arrangements.
- Production thumbnails must use the Canonical Slide Renderer. The CSS-drawn
  slide approximations in these explorations are illustrative only.
- Atkinson Hyperlegible Next with Noto Sans fallback is adopted. Arial Black
  controls in an exploration are retired.
- Production layout is qualified specifically at 1366x768 and 1920x1080;
  embedded-demo breakpoints are not application requirements.
