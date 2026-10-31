---
id: 258
title: "Bound live insertion, auto-play, windows, and save safety"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Bound live insertion, auto-play, windows, and save safety

- This extends Decisions [11](0011-program-preview-freeze-and-blackout-semantics.md)–[12a](0012-safe-live-operations.md), [54](0054-presenter-startup-and-informal-rehearsal.md), [113](0113-mass-editor-autosave-revision-grouping-and-undo.md), and [203](0203-same-window-native-full-screen-presenter-mode.md) without changing their
  unaffected live-safety, autosave, or same-window invariants.
- `Insert` Freezes and opens an anchored chooser for General text/image,
  Response sequence, or Song. Insert immediately after the current Mass item in
  the same section, select its first projectable Preview on save, and leave
  Program frozen until Space. Missing Songs created live are Mass-local first
  and may be promoted later.
- Emergency edits and inserted content autosave to the Mass plan by default.
  Updating a reusable source is a separate reviewed action and never rewrites
  immutable snapshots.
- Auto-play starts only by explicit operator action and targets exactly one
  General Presentation item or one Mass section. `A` toggles the item scope;
  the visible menu offers **Loop this section**. Never loop the complete plan or
  an arbitrary selection. Timed Songs are post-MVP.
- `A` is active only while focus is within the Cue timeline or Output Dock and
  no text editor, emergency editor, or true modal owns focus. Passive Ordo-rail
  and shortcut-reference focus suppress it while leaving `F`, `B`, and Space
  active.
- The reusable General Presentation owns an eight-second default duration,
  inherited by its Slides with optional Slide overrides. Every newly shown
  Slide receives its full timer, including after manual Previous/Next, and the
  last wraps to the first in scope. Manual navigation outside scope, Freeze,
  Blackout, leaving Presenter, or recovery stops and resets to manual.
- Keep one operator window. Presenter transforms it; emergency editors are
  slideovers and only congregation output uses another window. Presenter with
  output disabled is the rehearsal workflow, not a separate Rehearse mode.
- Back, close, and application exit flush pending autosave. On failure, block
  navigation with Retry save, Keep editing, and destructive Leave without
  saving; a successful retry resumes the original action.
