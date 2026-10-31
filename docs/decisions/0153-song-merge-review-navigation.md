---
id: 153
title: Song-merge review navigation
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Song-merge review navigation

- Treat Song merge as one linear two-step workflow inside the same full-app floating slideover: **Choose songs** followed by **Review merge**.
- Advancing to review replaces the main slideover's contents rather than opening another top-level slideover. **Back** returns to candidate selection while preserving the selected Songs and survivor assignment.
- Organize the main review around the surviving identity and metadata, Arrangements moving under it, and preserved references/redirect consequences.
- Reserve a Laravel Cloud-style nested child slideover for inspecting one specific conflict or comparison, such as two similar Arrangements. While the child is focused, recess the parent review visually without discarding its state or scroll position.
- Returning from the child comparison restores the same review position. Opening or closing either review surface remains non-mutating; only the eventual final merge commit may alter library state.
