---
id: 231
title: Later-discovered Package and Backup availability versus damage
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Later-discovered Package and Backup availability versus damage

- Use **Unavailable** when a known Celebration Package or Parish Backup location cannot currently be accessed, commonly because its removable drive is absent. Make no corruption claim without reading and verifying the artifact.
- Use **Damaged** only when an accessible artifact fails checksum, archive, manifest, dependency, snapshot, or stored-data verification. Keep the artifact visible in History with its exact failed checks; never silently delete or relabel it as a generic missing file.
- Prevent a Damaged artifact from import, restore, presentation, or qualification as current portable-backup protection. Never attempt archive surgery by discarding failed entries, rewriting the manifest, or treating the surviving subset as valid.
- Offer **Locate another copy**. Replace the retained artifact atomically only when the selected copy verifies with the exact artifact identity and complete fingerprint; otherwise route it through the ordinary new import or Package-version workflow.
- For a Damaged Parish Backup, also offer **Create new backup** when the current Parish scope passes integrity checks. If the current Parish is itself unhealthy, require another independently verified Backup rather than manufacturing protection from damaged state.
- Verify the retained artifact, extracted installed dependencies, ordinary imported Parish records, and independently stored Presentation snapshots as separate managed copies. Damage to a retained Package file does not cascade into otherwise verified local records or frames; a damaged snapshot affects only that snapshot and not sibling Mass plans or Package versions.
- Keep current Parish-store corruption under Diagnostics and Parish recovery. Do not mislabel it as Celebration Package or Parish Backup damage merely because those artifacts may be possible recovery sources.
