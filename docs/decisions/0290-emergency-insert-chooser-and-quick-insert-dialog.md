---
id: 290
title: Emergency insert chooser and Quick Insert dialog
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/presenter-insert.html]
---

# Emergency insert chooser and Quick Insert dialog

## Context

Decision [12](0012-safe-live-operations.md) makes `Insert` Freeze Program and open emergency
insertion. Decision [258](0258-bound-live-insertion-auto-play-windows-and-save-safety.md) adds an
anchored chooser for General text or image, Response sequence, or Song, inserting after the current
Mass item. MVP-PLAN §8.5 makes the Song path a centred, keyboard-first **Quick Insert** chooser built
from existing Field, list, Dialog and focus behaviour. None of them fixes what the chooser and dialog
contain or how the operator reaches them without the keyboard.

## Decision

- **Opening.** `Insert`, or **Insert after {item}** above the Cue timeline, opens the chooser. If
  Program is Live, it Freezes first, with the status **FROZEN · inserting**. With output disabled
  (rehearsal) nothing freezes, and the chooser says so.
- **Chooser.** It is anchored at the top of the window. It opens with the destination, **Inserts
  after {Mass item} · {section}**, then three choices with their keys: **Song** `S`, **Response
  sequence** `R`, **Text or image** `T`. `Esc` closes it and leaves Program frozen, with a message
  that Space or `F` takes Preview live.
- **Quick Insert.** A centred dialog with one search field focused.
  - With nothing typed, it lists the plan's choir's repertoire.
  - Typing filters by title first, then other names and first lines, ignoring accents. A result that
    matched on something other than its title shows why.
  - `↑`/`↓` move the active result, and its first slides preview beside the list. `Enter` or
    **Insert Song** inserts it.
- **No match.** Quick Insert offers **Type "{query}" for this Mass**. It opens a slide-over with the
  title and lyrics (a blank line starts a slide) and a live canonical preview. The Song is Mass-local,
  and its title slide shows no choir or "Mass-local" label to the congregation.
- **Response sequence and Text or image.** These open the same slide-over. A Response sequence is
  picked from Responses & Prayers and copied, leaving the Library entry unchanged. Text has a heading
  and body; an image is picked from this computer. Each shows a canonical preview before **Insert and
  preview**.
- **After inserting.**
  - The new item sits after the current Mass item in the same section, marked **Inserted live** in the
    timeline and the Ordo rail.
  - Its first projectable Cue is selected in Preview, and Program stays frozen (**FROZEN · Insert
    ready**) until Space.
  - A message confirms that it was saved to this Mass plan only.
- **Keys.** While the chooser, dialog or slide-over is open, no single-key Presenter command runs,
  including `F`, `B`, Space and navigation (MVP-PLAN §8.2).

## Alternatives considered

- **A global command palette.** MVP-PLAN §8.5 rules it out. A palette would also blur the difference
  between live commands and content.
- **Inserting after the current Cue instead of the item.** It would split a Song or Response sequence
  in two; Decision 258 already chose the item boundary.

## Consequences

The Library list, search and preview used in preparation are reused live. Post-Mass promotion of a
Mass-local Song stays a separate reviewed action.
