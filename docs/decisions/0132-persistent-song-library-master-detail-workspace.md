---
id: 132
title: Persistent Song Library master-detail workspace
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, storage, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Persistent Song Library master-detail workspace

- Use a desktop master-detail workspace for **Songs** at the supported `1366x768` and larger operating sizes rather than navigating between a full-width result page and Song detail.
- Keep the left browser pane responsible for the sticky Songs heading, **New song**, search, filters, visible result count, and Song-work results. Keep its results independently scrollable when the library exceeds the available height.
- Keep the right detail pane responsible for the selected Song work's canonical identity, attribution and metadata summary, reusable Arrangement list, and Song-level actions.
- Let selecting a result update the right detail pane in place without resetting the active query, filters, result scroll position, or search-field focus unnecessarily.
- Opening an Arrangement transitions into its full Arrangement editor. Returning to Songs restores the prior query, filters, selected Song, and result scroll position rather than reconstructing a default library view.
- Adapt below ordinary desktop width by progressively stacking the panes; do not replace the primary desktop design with a card grid.
