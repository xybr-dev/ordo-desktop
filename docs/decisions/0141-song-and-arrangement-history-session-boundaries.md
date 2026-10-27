---
id: 141
title: Song and Arrangement History-session boundaries
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

# Song and Arrangement History-session boundaries

- Start a History editing session with the first committed change after the operator opens a Song or Arrangement editor. Opening and inspecting an entity without changing it creates no empty History session.
- Keep the same Arrangement session across **Lyrics** and **Sequence** navigation because those tasks modify one Arrangement identity.
- End the session when the operator navigates to another Song or Arrangement, closes Ordo, or reaches 30 minutes without a committed change. A later edit starts a new session even if the same editor remained visible.
- Split standalone creation, import, whole-Arrangement replacement, and historical restoration events from surrounding ordinary edits so their provenance remains independently visible.
- Base session grouping on the edited entity. Song-work detail edits and edits to one of its Arrangements never collapse into the same History session.
