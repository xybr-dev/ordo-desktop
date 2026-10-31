---
id: 40
title: External extraction and Song import contract
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

# External extraction and Song import contract

- Ordo does not interpret arbitrary PowerPoint decks. External tools or AI agents extract heterogeneous parish and choir decks into versioned **Song import documents**.
- The Song import document is the strict contract at the import Seam. The MVP accepts multiple `.ordo-song.json` files in one import operation.
- One document proposes one Song work and includes its schema version and stable identity, primary and alternate titles, composer/source/language/provenance metadata, and one or more choir-specific Arrangements.
- An Arrangement in the document contains reusable song sections, its performance sequence, choir and occasion-applicability metadata, and optional extraction notes or confidence warnings.
- An extractor may group confidently recognized choir renditions as Arrangements under one Song work. When identity is uncertain, it emits separate candidates and leaves consolidation to Ordo's staged duplicate review.
- Imported pagination begins **Needs Review**. Preserved PowerPoint line or slide breaks are extraction hints, not automatically approved canonical pagination.
- Import validation and duplicate staging occur before any parish-library mutation. Ordo recommends **Add as new**, **Attach Arrangement to existing Song**, or **Skip exact duplicate** for each candidate; the operator may revise the recommendation before the approved batch is committed atomically.
- A ZIP song bundle is deferred until an import needs to carry images or other external assets; text-first migration uses multiple JSON documents directly.
- Song import documents require stable UUIDv4 identities for the proposed Song work, Arrangements, and other addressable imported records.
- Importing the same unchanged identities and content is recognized as an already imported document and skipped rather than duplicated.
- When an existing identity arrives with changed content, Ordo shows a comparison and requires a reviewed update decision; it never silently overwrites the parish library.
- Accepting an import update creates ordinary library edit revisions so the previous state remains restorable.
