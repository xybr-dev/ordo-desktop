---
id: 114
title: Arrangement-free Song creation and nested duplicate preview
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Arrangement-free Song creation and nested duplicate preview

- Let the operator create a canonical Song work before creating any Arrangement. **Primary title** is the only required field; composer/original author, aliases, usual liturgical moments, occasion tags, provenance, and rights notes are optional.
- Keep Arrangement-owned fields out of New Song creation: no lyrics, language, Choir association, reusable sections, performance sequence, or slide boundaries are requested.
- Use an explicit **Create song** commit so abandoning the New Song form does not leave an empty library record. After creation, select the new Song overview and show a valid **No Arrangements yet** state with **Add first Arrangement** as the next optional action.
- Present possible duplicate Song works through a compact, collapsed disclosure such as **2 possible existing Songs**. Candidate rows do not permanently occupy the creation form.
- Expanding the disclosure shows concise candidate identities. Selecting one opens a nested, viewport-bounded preview while the New Song panel recedes; it does not navigate to the complete Song page.
- The nested preview shows only evidence needed to distinguish identity: title and aliases, composer/source, usual liturgical moments, and a compact Arrangement/Choir/language summary. Arrangement lyric excerpts may appear only when needed as match evidence.
- Back from the nested preview restores the New Song form exactly as entered. **Use this existing Song** abandons creation deliberately and selects that Song in the library; merely previewing never alters the form or library.
- After creation, ordinary Song-detail edits use library autosave and revision behavior.
