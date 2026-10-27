---
id: 143
title: Song and Arrangement revision comparison and restoration
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Song and Arrangement revision comparison and restoration

- Selecting an individual revision opens a nested comparison slideover while the parent History surface recedes and retains its expanded sessions and selection.
- Compare the current entity with the selected historical revision without mutating either. Song comparison covers identity and metadata; Arrangement comparison provides **Overview**, **Lyrics**, **Sequence**, and **Slides** views consistent with Song-import comparison language.
- Emphasize differences and keep unchanged content available on demand. Use the authoritative canonical frame for historical Arrangement slide previews.
- Restore the entire Song or Arrangement revision in the MVP. Do not add field-, section-, line-, occurrence-, or slide-level cherry-picking.
- For an Arrangement restore, disclose associated Choirs and editable Mass plans that will observe the restored reusable content. Preserve Mass-local overrides and immutable Presentation snapshots unchanged. A Song restore changes only that Song work's identity and metadata and does not rewrite its Arrangements.
- Make restoration a two-stage action in the comparison footer. **Restore this revision** transforms the entire footer into a warning-tinted confirmation state with the explanation on its own row and **Keep current** plus **Restore entire Arrangement/Song** on a separate action row.
- Do not enclose the confirmation in another card. **Keep current** returns the footer to its initial state without changing content.
- A confirmed restore creates a new latest Edit revision and leaves all intervening history intact. Show immediate success feedback with **Undo**; Undo likewise creates a later revision.
