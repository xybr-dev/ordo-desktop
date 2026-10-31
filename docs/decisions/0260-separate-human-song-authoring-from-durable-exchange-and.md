---
id: 260
title: Separate human Song authoring from durable exchange and defer the ERD
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Separate human Song authoring from durable exchange and defer the ERD

- This supplements Decisions [16](0016-structured-song-import-boundary.md)/[40](0040-external-extraction-and-song-import-contract.md) and [135](0135-automatic-first-song-import-review-and-content-comparison.md)–[138](0138-song-and-arrangement-autosave-boundaries.md); `.ordo` remains the machine
  contract while Markdown is an editor-entry format.
- Accept `.md` and `.txt` as human-authored input for one Song plus one
  Arrangement, opening directly into the unsaved editor with inline validation
  and duplicate review. Retain `.ordo` as the versioned machine exchange and
  history container with stable identity and idempotent re-import.
- Use YAML front matter with required `title` and optional `aliases`, `composer`,
  `source`, `rights`, `arrangement`, `language`, `choirs`, `translator`,
  `adapter`, `liturgical_moments`, `occasions`, and `notes`.
- A blank line is a hard Slide boundary within the current labeled section.
  Labels persist across Slides; `[Unlabeled]` starts label-free content.
  `[Repeat: label]` is a linked occurrence, may refer forward or backward, and
  matches case-insensitively after trimming. Missing or ambiguous references
  remain visible and make the draft not presentation-ready without preventing
  draft save.
- Do not create a speculative ERD before persistence exists. Add `ERD.md` with
  the SQLite migration task; make it a physical schema reference whose Mermaid
  overview and per-table details correspond directly to authoritative
  migrations, while `CONTEXT.md` remains the domain glossary.
