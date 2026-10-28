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
Mass item, and makes emergency editors slide-overs. MVP-PLAN §8.5 describes the Song path as a
centred, keyboard-first **Quick Insert** chooser. A centred dialog for Songs and slide-overs for the
other kinds would give the same task two shapes. Both shapes would also cover Program and Preview at
the moment the operator most needs to see them.

## Decision

- **One Insert panel for every kind.**
  - `Insert`, or **Insert after {item}** pinned above the Cue timeline, opens it.
  - If Program is Live, it Freezes first, with the status **FROZEN · inserting**. With output disabled
    (rehearsal) nothing freezes.
  - The panel covers the Cue timeline only. Program, Preview, output status, Freeze, Blackout and the
    primary control stay visible and usable by mouse.
- **Panel layout.** The header names the destination: **After {Mass item} · {section}** and the
  Program state. Tabs choose the kind: **Song** (`Alt+S`), **Response sequence** (`Alt+R`), **Text or
  image** (`Alt+T`). It opens on Song with the search field focused.
- **Song and Response sequence** share one keyboard-first pattern: a search field, a result list and
  a canonical preview of the active result.
  - `↑`/`↓` move and `Enter` inserts.
  - Operator-only prompts appear in the preview as private rows, not slides.
- **Song search.**
  - The whole Library is searchable, by title first, then other names and first lines, ignoring
    accents. A match on something other than the title shows why.
  - Results are grouped: the plan's choir's repertoire first, then **Other Songs in the Library**.
  - A Song outside the repertoire is marked **Not in {choir}'s repertoire** when active. Inserting it
    doesn't add it to the repertoire.
- **No Song matches.** The panel offers **Type "{query}" for this Mass**. It opens a form in the same
  panel, with **Back to search** (or `Esc`), title, lyrics (a blank line starts a slide) and a live
  preview. The Song is Mass-local, and its title slide shows no choir or "Mass-local" label to the
  congregation.
- **Text or image.** A heading and text, or a picture from this computer, with a canonical preview.
- **After inserting.**
  - The new item sits after the current Mass item in the same section, marked **Inserted live** in the
    timeline and the Ordo rail.
  - Its first projectable Cue is selected in Preview, and Program stays frozen (**FROZEN · Insert
    ready**) until Space.
  - A message confirms that it was saved to this Mass plan only.
- **Keys.** While the panel is open, no single-key Presenter command runs, including `F`, `B`, Space
  and navigation (MVP-PLAN §8.2). `Esc` closes the panel and leaves Program frozen, with a message
  that Space or `F` takes Preview live.

## Alternatives considered

- **A centred Quick Insert dialog for Songs and slide-overs for the rest** (MVP-PLAN §8.5 as
  written). The two shapes differ for the same task, and both hide the Output Dock.
- **A global command palette.** MVP-PLAN §8.5 rules it out. A palette would also blur live commands
  and content.
- **Searching only the choir's repertoire.** That fails exactly when an unplanned Song is requested.
  Grouping keeps the repertoire first without hiding the rest.

## Consequences

If accepted, MVP-PLAN §8.5's "centered" Quick Insert wording is updated to this panel. The Library
search and preview used in preparation are reused live. Post-Mass promotion of a Mass-local Song stays
a separate reviewed action.
