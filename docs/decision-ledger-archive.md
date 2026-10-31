# Decision ledger archive

Not normative. These are the parts of the former single-file ledger `decision.md` that were not
numbered decisions, kept verbatim as they stood when the ledger was split into
[`decisions/`](decisions/README.md) on 2026-10-27
([0270](decisions/0270-record-decisions-one-per-file.md)). The decisions themselves are records
0001–0269 there. Domain terms are owned by `CONTEXT.md`, scope by `MVP-PLAN.md`, and task status by
`MILESTONES.md`; where this archive disagrees with them, they win.

---

## Ledger header

Status: MVP design baseline finalized; Rust/GPUI architecture locked; T0.1 complete; T0.2 active
Recorded: 2026-08-23  
Checkpoint: Functional, visual, operational, architecture, navigation, and execution-plan branches are finalized through Decision 260
Next interview branch: None required for the MVP baseline; execute `MILESTONES.md` T0.2 or deliberately reopen a named design branch
Planning deliverables: `MVP-PLAN.md` for strategy and `MILESTONES.md` for task execution
Supersession rule: Every superseding decision names the affected earlier decision; each affected decision's own section carries a forward notice stating whether replacement is full or partial and what remains authoritative.

## Product intent

Ordo is liturgical presenter software for Catholic audio-visual operators. Its first release is not merely a song database or a slide player: it must let one parish prepare and run a Sunday or solemnity Mass end-to-end without PowerPoint.

The central product idea is to keep the operator's liturgical context separate from what the congregation sees. A plan contains songs, spoken cues, congregation responses, readings, refrains, holds, blackouts, and other ordered items. During presentation, the operator sees the full order and upcoming context while the projector receives only congregation-facing output.

## Source evidence reviewed

- Thirteen PowerPoint files were inspected read-only: 796 slides total.
- The twelve numbered song decks encode a partial Mass sequence and contain roughly 83 song or setting candidates.
- A follow-up pagination analysis examined 502 congregation-facing lyric slides across the twelve song decks, excluding titles, blanks, notes, and the prayer/response deck:
  - 21% contain one explicit/logical lyric line, 54% contain two, 12% contain three, 12% contain four, and only 1.4% contain five or more.
  - The median and mode are two logical lines; 98.6% contain four or fewer.
  - 500 of 502 slides inherit 54-point Arial Black; the two exceptions use 48-point text.
  - Long logical lines often soft-wrap, producing an estimated median of about four rendered lines; roughly 14% likely exceed four rendered lines.
  - Prose-like settings such as the Our Father and Memorial Acclamation need stronger automatic splitting than ordinary song verses and choruses.
- `2025 Prayers and Tubag Bisaya.pptx` contains congregation responses, private priest cues in speaker notes, choir-dependent instructions, meaningful blank/hold slides, and non-liturgical announcements.
- The only dated Markdown file is `2026-08-23.md`. The prompt's `2026-08-03.md` was a filename mismatch; no such file currently exists.
- `2026-08-23.md` is a Cebuano Liturgy of the Word source containing the first reading, responsorial Psalm, second reading, and Gospel. It is not a complete Mass plan.
- Sulugoon's dated readings and Order of the Mass were reviewed as references:
  - <https://sulugoon.com/readings/2026/08/23/>
  - <https://sulugoon.com/missal/>
- Sulugoon uses role labels including `P` for `Pari` (Priest) and `T` for `Tanan` (the whole congregation), and includes conditional rites, alternatives, rubrics, and non-projected priest text.
- Canon 1248 §1 was reviewed as official support for distinguishing a Celebration's liturgical date from a Mass held on the evening of the preceding civil day; Ordo still does not infer the correct texts or local scheduling rule from canon law.

## Final design-status summary

The following branches are resolved for the MVP baseline:

1. **Presenter command chrome:** completed—command ownership, placement, Ordo-rail access, discoverability, focus return, and shortcut suspension are adopted.
2. **Mass Planning workspace:** completed for the MVP—the focused three-pane hierarchy, floating Arrangement picker, top-right Add Mass item placement, owner-directed pagination, Inspector Cue navigation, editable six-section scaffold, section behavior, and Presenter handoff are adopted.
3. **Song Library and Arrangement Editor:** completed for the MVP—Arrangement-free Song creation, collapsed/nested duplicate preview, the persistent Song Library master-detail workspace, normalized Song/Arrangement/lyrics search, Arrangement-level Choir/language/occasion filtering, hierarchical automatic-first atomic Song import review with read-only content comparison and explicit near-duplicate Arrangement revision boundaries, Song/Arrangement autosave boundaries, editor-local immediate Undo, fine-grained durable revisions with deterministic History-session grouping, entity-owned History surfaces, nested whole-entity comparison and guarded restoration, independent non-cascading Arrangement archival, inspect-only archived state, reference-safe non-cascading permanent deletion, preservation of archived Arrangement references in Mass plans, and the complete Lyrics/Sequence authoring workspace are adopted. Song merge and its explored reversal model are retained for post-MVP work only.
4. **Pagination handling and Theme Editor:** completed for the current scope—pagination correction routes directly to the owning editor rather than a standalone workspace; Mass Theme selection, override review, dedicated Theme Editor navigation, canonical specimen previews, Typography/Appearance/Layout controls, staged shared editing, revision-based Save/Undo, exception-based affected-plan review, solid/image backgrounds, dirty-session navigation, contextual duplication, recoverable library management, and visible revision history are adopted.
5. **Transfer, backup, and settings workspaces:** completed for the current scope—Celebration Package and Parish Backup creation/restore, manual-only safety-Backup retention, installation-specific Displays and calibration, same-window native-full-screen Presenter transformation, and offline Diagnostics with deliberate sanitized export, explicit scan lifecycle, and issue-first hierarchy are adopted.
6. **Shared visual tokens:** completed—the coordinated warm-paper/graphite appearances, semantic colors, Google Labs-compatible `DESIGN.md`, interaction-state behavior, focus, separators, corner geometry, elevation, iconography, adaptive spacing/density, and functional motion are adopted.
7. **Non-happy-path screens:** completed for the MVP—first-run, empty/fallback-only workspaces, strict import states, asset/font recovery with installer-owned repair, safe uninstall retention, fail-safe unavailable-Cue behavior, disconnected-output recovery, interrupted-session presentation, and later-discovered Package/Backup damage are adopted.
8. **Accessibility and keyboard review:** completed for the MVP—visible focus, non-color state foundations, reduced-motion tokens, contextual shortcut discovery, complete multi-pane focus order, operator-only interface scaling isolated from canonical Slide composition, and concise safety-prioritized screen-reader semantics are adopted.
9. **Responsive visual acceptance:** completed for the software references—the responsive rules, constrained/high-scale Presenter reference, and paired `1366x768`/`1920x1080` Mass Planning reference are approved. Physical church Presenter/projector qualification remains the final operational branch and eventual release gate, administered through the external release checklist rather than Ordo UI.
10. **Physical church qualification design:** completed—the external two-pass gate, risk-based requalification, privacy-safe evidence, operator/observer roles, sanitized two-plan fixture, `60`-minute minimum normal run, and blocking-failure boundary are adopted. Execution remains an implementation/release activity on the finished release candidate and actual church system.
11. **Desktop shell research:** completed—Rust + GPUI supersedes Electron + React as a clean-slate implementation. Performance and cross-platform canonical-rendering gates are mandatory, and no dual-shell bake-off is planned.
12. **Preparation-shell navigation:** completed—the global vertical application rail distinguishes destinations from workspace-local horizontal tabs, adapts to effective width/scale, and disappears when the main window transforms into Presenter Mode.
13. **Implementation execution plan:** completed and reconciled to the GPUI restart—`MILESTONES.md` owns detailed ordered tasks, checkbox acceptance criteria, dependencies, test-mode tags, E2E scenarios, and phase gates, while `MVP-PLAN.md` remains strategic.
14. **Native workspace locality:** completed—the initial short-named `domain`, `storage`, `rendering`, `presentation`, `ui`, `platform`, and `ordo` crates enforce the major dependency directions; `tools/ui-lab` provides an internal interactive and automated primitive proving ground; later crates must earn extraction through a deep Interface.
15. **Agent-development system:** completed—the production repository owns its implementation documentation and agent guidance; a small external Rust/GPUI skill baseline complements focused repository skills created only from proven repetition; `cargo xtask` is the initial offline Ordo-aware inspection and verification surface.
16. **Canonical Slides and reusable content:** completed for behavior—the fixed semantic Slide templates, bounded blank fades, Preparation appearance preference, multi-view Library, plan-level reusable-source bindings, reviewed promotion/archive rules, General Presentation auto-play, live insertion, Reading reuse review, Song Markdown authoring, and migration-owned ERD are adopted. Exact projected type sizes remain intentionally provisional until the Canonical Slide Layout Gallery is qualified.

## Completed Mass Planning interview record

The Mass Planning branches below are finalized for the MVP; no unresolved queue remains here:

1. **Mass outline header:** completed—its title and **+ Add Mass item** are the only MVP actions; an outline-wide `...` menu is deliberately omitted under YAGNI.
2. **Mass-item interactions:** completed—single selection, unrestricted handle/keyboard reordering, one shared row/context menu, explicit non-destructive removal wording, autosave, and Undo are adopted; multi-selection remains deferred.
3. **Inspector ownership:** completed—the ambiguous Inspector `...` is removed; canonical-slide navigation, warning state, Theme override, and contextual **Fix pagination** remain explicit, while outline and editor actions stay on their owning surfaces.
4. **Readiness and warnings:** completed—the header exposes explicit incomplete/warning counts; a floating Readiness panel groups computed findings, navigates or focuses corrective controls, and resolves automatically; incomplete output requires acknowledgment but only snapshot-level fatal failure blocks Presenter Mode.
5. **Mass Theme control:** completed—a viewport-bounded floating chooser previews and stages a parish Theme; a Laravel Cloud-style nested slide-over reviews `current override → inherited result`, and one commit applies the Theme choice and staged override removals coherently. **Edit selected theme** routes to the adopted dedicated Theme Editor and preserves staged chooser context on return.
6. **Mass metadata and templates:** completed for the current scope—a language-neutral Celebration may own multiple independent scheduled Mass plans, including anticipated plans on the preceding civil day and several plans in one language; conditional items, typed Song/Reading slots, multiple templates per language, **Masses → Templates** ownership, independently copied structure with retained reusable-content links, visible applicability-based template recommendation with one fallback per language, independently authored language-specific liturgical content, language-aware duplication, a two-section Mass-details slide-over, a Celebration-grouped chronological index, optional two-stage Celebration/first-plan creation, minimal metadata without liturgical grade/rank, lightweight Presider/Location input with non-retroactive parish defaults and regular-Presider suggestions, and non-destructive shared-edit review are adopted.
7. **Selected-item editor:** completed for the current scope—the Song editor's plan/library ownership boundary, explicit Mass-local customization state, independent copy boundary, linked repeated-section semantics, guarded replacement behavior, expandable section-occurrence/generated-Cue hierarchy, optional Song-level identification Cue, linked reusable Response-sequence exchange blocks, consistent autosave/Undo behavior, and owner-directed pagination correction are adopted.
8. **Mass-section grouping:** completed—the six-section default scaffold is editable and independently copied; sections use UUID-plus-label identity without inferred liturgical behavior, have informative authoring-only collapse, explicit insertion targets, hierarchical reordering, scoped header actions, non-cascading removal, unique nonblank names, and a minimum one-section/exactly-one-membership invariant.
9. **Inspector Cue navigation:** completed—the selected Mass item's flattened Cue order drives a synchronized Inspector; projectable Cues show canonical output, operator-only triggers show black with private context outside the frame, and navigation stops at item boundaries.
10. **Presenter handoff:** completed—the exact **Present** action, automatic candidate validation and snapshot reuse/creation, clean/warning direct entry, centered incomplete-content acknowledgment, fatal blocking, output-disabled entry, interrupted recovery, and safe planned exit/return behavior are adopted.

## Working domain language

- **Ordo:** The ordered structure of one prepared Mass. Context must distinguish this domain object from the product name, Ordo.
- **Celebration:** The dated liturgical occasion and its calendar metadata.
- **Mass plan:** A parish-specific prepared instance for a Celebration, built from a template and containing ordered items.
- **Reuse for another Celebration:** Creation of a new, independent Mass plan from an existing plan, assigned to a different Celebration and date while leaving the source plan unchanged.
- **Mass item:** A top-level titled unit in the plan, such as an entrance song, Greeting, reading, Psalm, response sequence, announcement, or blackout/hold.
- **Mass section:** An ordered named grouping of Mass items representing a large-scale phase of the prepared Ordo. It has no Cue or projectable output of its own and is distinct from a Song's Arrangement section.
- **Cue:** An ordered operator-relevant event within a Mass item. A cue may or may not produce congregation output.
- **Wait Cue:** An operator-relevant Cue that deliberately leaves Program unchanged.
- **Operator-only trigger:** A priest/minister phrase or action used to time the next projected response.
- **Canonical logical slide canvas:** The fixed `1920x1080` coordinate space in which every congregation-facing slide is composed and text is wrapped before resolution-independent scaling.
- **Projectable output / slide:** A rendered 16:9 congregation-facing frame.
- **Program:** Exactly what the congregation display currently shows.
- **Preview:** The operator's selected current/upcoming rendered output.
- **Congregation output disabled:** A Presenter Mode state in which no congregation-output window exists while operator Preview and navigation remain available; it is distinct from Blackout.
- **Freeze:** Hold Program while allowing Preview navigation.
- **Blackout:** Force Program to black while allowing Preview navigation.
- **Song work:** The canonical identity of a song independent of choir, arrangement, language version, or liturgical use.
- **Merged Song redirect (post-MVP):** The retained identity of a Song work retired by a reviewed merge, resolving its old UUID, titles, and aliases to the current survivor without remaining independently selectable.
- **Occasion applicability tag:** Manually reviewed metadata indicating that a Song is suitable for or conventionally used during a liturgical season or special occasion. It is not a claim that the Song's numbered Mass label inherently denotes that season.
- **Arrangement:** An independent choir- or season-specific rendition of a Song work, containing its own reusable sections, performance sequence, reviewed pagination, and applicability metadata.
- **Source reference:** Informational provenance for parish content, such as its origin, attribution, and retrieval date; it does not lock the editable parish copy.
- **Canonical rendered frame:** The single composition produced from the canonical logical slide canvas and shared—only scaled or rerendered at a different pixel density—by the timeline thumbnail, Preview, Program, congregation display, and image-based fallback export.
- **Mass-local override:** A live correction stored only with the current Mass plan until explicitly promoted through post-Mass review.
- **Reusable-source binding:** The plan-level association that keeps every still-linked occurrence of one reusable Library source on the same Edit revision.
- **General Presentation:** A reusable constrained Text or Image sequence intended for announcements and other non-liturgical congregation output.
- **Auto-play scope:** One explicitly started General Presentation item or Mass section whose eligible Slides loop until manual control resumes.
- **Song import document:** A versioned JSON representation of one canonical Song work, its reusable sections, metadata, and one or more Arrangements.
- **Liturgy Import Document:** A versioned, source-neutral representation of externally prepared content for one Celebration, validated and previewed before it is copied into a Mass plan.
- **Live Control:** The operator-facing presentation screen shown inside the existing main window while Presenter Mode is active; it is not a separate operating-system window.
- **Presenter Mode:** The native-full-screen presentation state that transforms the existing main operator window into Live Control and may own one separate congregation-output window.
- **Presentation snapshot:** An immutable captured version of one Mass plan used for a Presenter Mode session or as one member of a Celebration Package.
- **Presentation session:** One live operator run of a Presentation snapshot, with its own Program, Preview, Freeze, Blackout, and recovery state.
- **Edit revision:** A restorable historical state of a Mass plan or reusable library entry; restoring one creates a later revision rather than deleting history.
- **Celebration Package:** A self-contained portable bundle containing one complete Celebration, one independent Presentation snapshot per included Mass plan, and all offline dependencies required to present them.
- **Parish Backup:** A complete portable snapshot used to restore or migrate a parish's Ordo data, settings, and media.

## Explicitly deferred from the MVP

- Multi-parish cloud tenancy and public self-service onboarding.
- Private same-parish cloud synchronization across parish installations.
- Full support for funerals, weddings, baptisms, novenas, and other special rites.
- Rehearsal management and choir-member proficiency tracking.
- Song-usage history and analytics.
- Separate granular staff roles and approval permissions.
- Automated recommendation engines.
- A first-class Mass-setting aggregate that groups related Ordinary movements and prefills them as a suite.
- Remote phone control.
- Audio playback as a core presenter feature.
- Song merging, retiring-Song redirects, merge History and reversal, and the associated conflict/dependency workflow.
- Linux preparation, the Linux Flatpak, and the Linux diagnostic ZIP (Decision 268).

## Grilling outcome

- All consequential functional MVP branches identified during the original interview are resolved.
- Song Library, Arrangement Editor, Mass Planning, Mass-section grouping, and Presenter handoff design are complete for the MVP. Celebration-scoped portable transfer now supports repeated self-contained preparation-to-church-to-preparation round trips while imported content remains ordinary local content; export edge cases are next.
- Deliberately deferred post-MVP topics remain listed above and do not block implementation planning.
- `MVP-PLAN.md` is finalized as the implementation-planning baseline.
