---
id: 103
title: "Theme library management, archival, and protected fallback"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Theme library management, archival, and protected fallback

- The top-level Theme library exposes **Edit**, **Rename**, **Duplicate**, **Set as parish default**, and **Archive** as the MVP management actions.
- **Archive** is the recoverable removal operation. An archived Theme is hidden from ordinary new Theme selection but remains available to existing Mass plans that inherit it and may later be restored.
- An archived Theme continues rendering existing editable plans and immutable Presentation snapshots coherently; archival never substitutes a different Theme into them.
- A Theme that is currently the parish default cannot be archived until the operator chooses a replacement default.
- Permanent Theme deletion is deferred from the MVP. This avoids breaking inheritance or prematurely defining asset-garbage-collection behavior.
- Ordo ships with a protected black-background/white-text safety Theme. It may be selected or duplicated, but it cannot be edited in place, renamed, or archived, ensuring that a dependency-free fallback always exists.
