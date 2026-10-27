---
id: 136
title: Near-duplicate Arrangement import conflicts
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

# Near-duplicate Arrangement import conflicts

- Classify an imported Arrangement that resembles an existing Arrangement but contains meaningful metadata, lyrics, section, Sequence, or pagination differences as **Needs decision**. Never silently attach it as a new variant or overwrite the existing Arrangement.
- Offer exactly three deliberate outcomes: **Add as new Arrangement**, **Update existing Arrangement**, or **Skip document**. Use **Add as new Arrangement** as the safe initial recommendation when Ordo cannot prove update intent.
- Treat **Update existing Arrangement** as whole-Arrangement replacement under the existing Arrangement identity, not as a field-by-field or line-by-line merge. Capture the replacement as a new restorable Edit revision.
- Before updating, disclose the existing Arrangement's associated Choirs and unsnapshotted Mass plans that will observe the revised reusable content. Preserve Mass-local overrides and immutable Presentation snapshots unchanged.
- Keep lyric, section, Sequence, and pagination comparison available as evidence before the operator chooses. Do not attempt automatic conflict resolution inside those structures in the MVP.
