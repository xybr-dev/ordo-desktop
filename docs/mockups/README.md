# Mockups

Open [`index.html`](index.html) in a browser. Each page is one screen, with its status, the
decisions behind it, and any details a later decision replaced. The layout follows clockout's
`docs/mockups/` ([0271](../decisions/0271-mockups-are-one-page-per-screen-with-a-status-each.md)).

These pages illustrate the normative documents and yield to them: `DESIGN.md` owns visible
behaviour and tokens, `CONTEXT.md` domain terms, `MVP-PLAN.md` scope, `MILESTONES.md` acceptance.
They are not production source; screens are rebuilt as native GPUI views.

## Files

- `mockups.js` holds the **registry** (`PAGES`, `GROUPS`, `FLOWS`) and renders the index, each
  page's header and its notes from it. Status lives here and nowhere else.
- `mockups.css` is the chrome only: header, notes, index, status chips. Light uses the Preparation
  tokens and dark the Presenter tokens from `DESIGN.md`.
- `<slug>.html` is one screen: a fixed shell plus the screen's markup. Screen styles are scoped
  to the screen's root id so they can't touch the chrome.
- `ordo.css` is the **kit**: `DESIGN.md`'s tokens as CSS variables (`--o-*`) plus the shared
  components every regenerated screen is built from. Revising a token here restyles every `-v2`
  page at once. Preparation tokens apply to `.o-app`, graphite ones to `.o-presenter` (always) and
  to `.o-app` in dark mode.
- `ordo-kit.js` holds the sample Mass fixture and renderers, including `OrdoKit.presenter()`
  (layout, Cue timeline, Output Dock and the Presenter state machine with its keys). Frames are
  canonical 1920×1080 slides drawn in container units, so they scale and never reflow.
- `ordo-planner.js` adds `OrdoKit.planner()`, the Mass planner with its item editors and flows.
  `ordo-songs.js` adds `OrdoKit.library()`: the Library's Songs view, the Arrangement editor,
  Arrangement details, History and Song import review. Each instance keeps its own working copy of
  its fixture, so a page's states can edit and Undo without affecting one another.
  `ordo-shell.js` adds `OrdoKit.shell()`: the Masses index and Set up Ordo, Themes and the Theme
  Editor, Transfer & Backup, Diagnostics, Help and Settings. The rail switches between them in place.
  `ordo-templates.js` adds `OrdoKit.templates()`: Masses → Templates and the Template Editor.
  `ordo-liturgy.js` adds `OrdoKit.liturgy()`: the Liturgy import review.
- `canonical-slide-layouts.html` shows the PNG gallery in `design-references/`, which the
  `canonical-slide-lab` capture test writes there.

## Statuses

| Status | Meaning |
| --- | --- |
| `approved` | Adopted. The hierarchy and interaction stand, apart from listed `caveats`. |
| `proposed` | Awaiting a yes or no; nothing in it is adopted. |
| `superseded` | Replaced; `supersededBy` names the page that replaced it. Kept for history. |
| `research` | Explored and kept for after the MVP. |

Two flags sit beside the status: `gate: true` marks an *Acceptance reference* named in
`DESIGN.md`'s Approved References, and a non-empty `caveats` list shows *Partly superseded*.

## Adding or changing a page

1. Copy an existing page, change `data-page` and `<title>`, and put the screen's markup inside
   `<main class="mk-stage">`. Scope its styles under one root id.
2. Add an entry to `PAGES` in `mockups.js`, in reading order, with `status: "proposed"` and the
   `decisions` (record file stems) it rests on.
3. To approve it, write or accept the decision record first, then set `status: "approved"`.
   Approving a page never happens in the page alone.
4. To replace a page, add the new one as `proposed`; when it is approved, mark the old one
   `superseded` with `supersededBy`. A detail replaced by a decision goes into `caveats` instead.
   The newest page is canonical: an acceptance reference moves to it, and the index hides
   superseded pages until the *Superseded* or *All* filter is chosen (Decision 294).
5. A regenerated page is `<old-slug>-v2.html` on the kit, with `replaces: "<old-slug>"` and an
   `interpretations` list naming every place it fills a gap the decisions leave open.
6. A page that becomes an acceptance reference gets `gate: true` and a line in `DESIGN.md`.
