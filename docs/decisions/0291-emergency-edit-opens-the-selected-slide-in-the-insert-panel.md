---
id: 291
title: Emergency edit opens the selected slide in the Insert panel
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/presenter-edit.html, MVP-PLAN.md]
---

# Emergency edit opens the selected slide in the Insert panel

## Context

Decision [12](0012-safe-live-operations.md) and MVP-PLAN §8.5 define `Ctrl+E`. It Freezes Program,
edits the selected projectable Cue, and updates only Preview. The edit autosaves as a Mass-local
override and never changes the Library or imported text. A reused Song section updates every linked
occurrence in the Mass unless the operator limits it to the selected one, and Space takes the
correction live. Decision [258](0258-bound-live-insertion-auto-play-windows-and-save-safety.md)
makes emergency editors slide-overs. Proposed Decision
[290](0290-emergency-insert-chooser-and-quick-insert-dialog.md) puts emergency insert in one panel
over the Cue timeline, leaving the Output Dock in view. Neither record says what the edit surface
contains or what happens when `Ctrl+E` meets a Cue that can't be edited.

## Decision

- **Where.** `Ctrl+E`, or **Edit slide** pinned beside **Insert after {item}** above the Cue timeline,
  opens the Preview Cue in the same panel position as Insert. Program, Preview, output status,
  Freeze, Blackout and the primary control stay visible and usable by mouse.
- **Freeze.** If Program is Live, it Freezes first, with the status **FROZEN · editing**. Program
  keeps the frame it was showing, even when the edited Cue is the one on Program. With output
  disabled (rehearsal) nothing freezes.
- **What can be edited.** Only the fields the slide shows:
  - Song title slide: title and credit line.
  - Lyrics: section label and lyrics.
  - Psalm and Response: heading and text (plus reference for a Psalm).
  - Reading title and passage: text and reference.
  - General text: heading, text and footer.
  Splitting a slide, pagination and Theme are not edited live. The text field takes focus.
- **Header.** It names the Mass item, Cue number, kind and Program state, and where the words come
  from: **From the Library Arrangement {Song} · the Library isn't changed**, **From the imported
  liturgy · the imported text isn't changed**, **From Responses & Prayers**, or **already Mass-local**.
- **Saving.** Each keystroke autosaves a Mass-local override and updates Preview, in the panel and the
  Output Dock. The panel shows **Now in Preview** above the original (**Library** or **Before**). The
  footer confirms **Saved to this Mass plan · Mass-local**.
- **Linked occurrences.** When the Cue is a Song section sung more than once in this Mass, **Apply to**
  offers **All N "{section}" slides in this Mass** (default, listing the Cue numbers) or **Only this
  slide**.
- **Undo.** **Back to Library words**, **Back to imported text** or **Back to original**, depending on
  the source, restores the original words. It is itself saved, and Preview updates.
- **Closing.** `Esc`, `Ctrl+Enter` or **Done** close the panel and keep the edit. If anything changed,
  Program stays frozen with **FROZEN · Edit ready**, and a message says the change was saved to this
  Mass plan only and that Space takes it live. Edited Cues show **Edited**; the item shows **Edited
  live · Mass-local**, and the Ordo rail shows **Edited**.
- **Keys.** While the panel is open, no single-key Presenter command runs (MVP-PLAN §8.2,
  Decision [67](0067-presenter-shortcut-suspension-and-focus-restoration.md)).
- **Cues that can't be edited.** `Ctrl+E` opens nothing and doesn't freeze; the message says why:
  - an operator-only trigger, wait or hold (never shown);
  - an unavailable Cue (use Insert to put something in its place);
  - a picture (use Insert to add another).
  **Edit slide** is disabled while such a Cue is in Preview.

## Alternatives considered

- **A right-hand slide-over** (Decision 258 as written). It covers the Output Dock while live; see
  Decision 290.
- **Editing in place on the timeline tile.** It is too small to read at operator distance, and it
  can't show linked occurrences or the original beside the change.
- **Cancel discards the edit.** Autosave exists for crash recovery, so closing keeps what was typed.
  Undo is an explicit **Back to …** action instead.

## Consequences

If accepted, Decision 258's "emergency editors are slide-overs" is read as this panel, together with
Decision 290. Post-Mass review offers each override for promotion to the Library or source. Linking
occurrences needs Arrangement section identity; the mockup approximates it by matching the words.
