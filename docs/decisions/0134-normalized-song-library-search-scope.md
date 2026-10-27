---
id: 134
title: Normalized Song Library search scope
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

# Normalized Song Library search scope

- Search Song-work title, aliases, composer/original-author attribution, and source metadata, together with Arrangement display title, translator/adapter attribution, Choir, language, and full lyrics.
- Keep search results at the Song-work level. When a query matches Arrangement-owned data, require that same Arrangement to satisfy every active Arrangement filter; never satisfy the query with one Arrangement and the filters with another.
- When a query matches Song-work identity or attribution, retain the Song when it owns an Arrangement satisfying all active filters.
- Explain non-obvious results with a compact match reason, such as a matched alias or a short lyric excerpt. Order and emphasize the matching Arrangements under the adopted master-detail behavior.
- Normalize indexed text and queries using Unicode-aware normalization and case folding so search is case-insensitive across supported languages. Normalize inconsequential spacing consistently, but preserve meaningful letters and diacritics in stored/displayed content; normalization affects matching only and never rewrites authored text.
