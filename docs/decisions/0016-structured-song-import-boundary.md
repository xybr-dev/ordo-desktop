---
id: 16
title: Structured song import boundary
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Structured song import boundary

- Ordo will not import or interpret arbitrary PowerPoint files.
- Migration of the existing parish decks will be performed separately, potentially using AI agents and external extraction tooling.
- Ordo exposes a documented, versioned JSON import shape for structured song data.
- The application validates and previews this structured data before adding it to the library; it does not own arbitrary deck interpretation.
- One JSON document represents one canonical Song work and contains:
  - Schema version and stable import ID.
  - Title, aliases, language information, authorship/source, and rights metadata.
  - Eligible liturgical moments.
  - Optional manually reviewed liturgical-season and special-occasion applicability tags.
  - For numbered Ordinary movements, the identifying number in the title/aliases plus composer, collection, or source metadata when known; no parent Mass-setting record is required in the MVP.
  - One or more independent choir- or season-specific Arrangements.
  - Each Arrangement's applicability metadata, including choir and eligible liturgical seasons when known.
  - Each Arrangement's own reusable sections with explicit line and slide breaks, plus a performance sequence referencing those section IDs.
  - Optional theme overrides and importer notes.
- Ordo may accept multiple documents as one batch, but validates, previews, and approves each Song work independently.
- A matching title or alias never causes an automatic overwrite. The importer requires an explicit create, merge, or skip decision.
