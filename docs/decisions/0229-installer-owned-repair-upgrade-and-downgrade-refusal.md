---
id: 229
title: "Installer-owned Repair, Upgrade, and downgrade refusal"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Installer-owned Repair, Upgrade, and downgrade refusal

- Make running the complete Windows installer the ordinary application-repair path. A same-version installer offers **Repair Ordo** or performs an equivalent explicit repair, replacing application binaries, factory fonts, and other packaged resources while preserving all Parish data outside the installation directory.
- After same-version repair, rebuild the runtime font cache and run application-resource, font, and canonical-renderer verification automatically. Do not create a Parish Backup merely for repair because no authoritative stored data or schema changes.
- Treat a trusted newer installer as **Upgrade Ordo**, not as a source of files to inject into the older build. Preserve the adopted complete-installer offline update flow; on first launch, verify integrity and create the locally managed safety Backup before any required ordered schema migration.
- Refuse an older installer as an in-place downgrade and never extract its font pack into a newer build, even when font family names appear unchanged. Supporting arbitrary data/schema downgrade is outside the MVP and risks opening newer Parish state with older semantics.
- Defensively refuse to open a Parish database whose schema is newer than the running application, without migrating backward, rewriting, or partially opening it, even if an operator bypassed installer downgrade protection.
- For controlled unsigned MVP deployment, retain published checksum verification before copying the installer to air-gapped media. Once public distribution begins, also require the adopted trusted publisher/signature checks.
