---
id: 197
title: Complete Parish-scope Backup without installation cloning
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Complete Parish-scope Backup without installation cloning

- Keep the MVP **Parish Backup** complete and non-selective. Include the parish identity and settings; every Celebration, Mass plan, template, Mass-local change, Song, Arrangement, Choir, Theme, and custom asset; durable Edit revisions and restorable History; Presentation snapshots; Package versions and provenance; identity mappings, sync anchors, import records, and other authoritative state needed to preserve offline round-trip ancestry.
- Deduplicate repeated immutable assets by content identity inside the Backup without introducing external dependencies. A Parish Backup must remain self-contained and restore without internet access or access to the source installation.
- Exclude application binaries and bundled application resources, regenerable thumbnails/caches, temporary or staged files, diagnostic logs, and active Presentation-session recovery state.
- Preserve recognized-device display settings only as dormant source-device metadata. Activate them after restore only when the corresponding hardware is recognized; otherwise reset Output Fit safely and require calibration.
- Never restore the source computer's installation UUID onto the destination. A restore recreates the Parish scope while the destination remains its own installation, preserving meaningful revision ancestry and future synchronization evidence.
- Defer selective and incremental backup formats beyond the MVP. They are not variants of a Parish Backup in the current product language.
