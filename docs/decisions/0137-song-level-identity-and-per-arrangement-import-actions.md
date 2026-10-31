---
id: 137
title: Song-level identity and per-Arrangement import actions
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

# Song-level identity and per-Arrangement import actions

- Treat one Song Import Document as one internally coherent Song-work candidate that may contain several imported Arrangements.
- Resolve the document's Song-work identity once: either create one new Song work or associate the document with one existing Song work. Never attach different Arrangements from one document to several Song works.
- After Song identity is resolved, classify and review each imported Arrangement independently as a new attachment, unchanged skip, existing-Arrangement update, deliberate new variant, or unresolved decision under Decisions [135](0135-automatic-first-song-import-review-and-content-comparison.md)–[136](0136-near-duplicate-arrangement-import-conflicts.md).
- Preserve whole-document contract validity. If any Arrangement or internal reference is invalid, reject the entire document rather than recovering or importing a valid-looking subset in the MVP.
- Require every Arrangement decision within every otherwise valid approved document to be resolved before enabling the atomic batch commit.
