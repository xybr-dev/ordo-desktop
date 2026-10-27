---
id: 145
title: Song and Arrangement lifecycle-action placement
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

# Song and Arrangement lifecycle-action placement

- Keep frequent creation, editing, and recovery actions visually primary while placing low-frequency lifecycle actions in the owning entity header's overflow menu.
- In Song detail, expose **New arrangement** and **History** directly. Place **Merge with another song** and **Archive song** in the adjacent Song overflow, with Archive separated as the destructive lifecycle group.
- In the Arrangement editor, expose **Arrangement details** and **History** directly and place **Archive arrangement** in its adjacent overflow.
- Keep **Duplicate arrangement** in Arrangement Details, where its metadata and branching context are already visible, instead of duplicating it in the header overflow.
- For an archived Song or Arrangement, replace Archive with **Restore** and expose **Delete permanently** in the same overflow. Keep permanent deletion invokable even when references make it unsafe so Ordo can show the blocking references and recovery-safe alternative instead of silently hiding or disabling the action.
