---
id: 230
title: Uninstall preserves Parish data by default
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

# Uninstall preserves Parish data by default

- Present exactly two uninstall scopes with **Keep Parish data — Recommended** preselected. This removes application binaries and disposable runtime caches while preserving the Parish database, managed assets, settings, Presentation snapshots, Package history, and app-managed local Backups outside the installation directory.
- A later compatible reinstall discovers the retained Parish scope and opens it only after ordinary schema-compatibility and integrity checks. Same-version installation should normally expose Repair, so uninstall-first is not required for routine application repair.
- Offer **Remove all local Ordo data** as the explicit destructive alternative. Before commit, list the app-managed categories that will be deleted, show the latest verified portable Parish Backup, and warn clearly when current authoritative changes are not protected.
- Require one explicit danger confirmation for full local removal, but do not require typing the Parish name. Do not invent Undo; recovery afterward exists only through a surviving portable Parish Backup or Celebration Packages according to their ordinary scopes.
- Delete only Ordo-managed local data selected by that scope. Never search for or delete exported `.ordo` files in user folders, arbitrary filesystem locations, removable drives, or another installation's storage.
- Keep application uninstall and Parish-data deletion as separate installer operations internally so the safe default cannot accidentally broaden through a packaging flag or cleanup routine.
