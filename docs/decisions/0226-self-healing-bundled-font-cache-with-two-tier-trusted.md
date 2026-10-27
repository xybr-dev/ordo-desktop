---
id: 226
title: Self-healing bundled-font cache with two-tier trusted recovery
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering, storage, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Self-healing bundled-font cache with two-tier trusted recovery

- Ship a verified read-only factory font pack inside Ordo and load projection fonts from a separate versioned runtime cache. Verify the runtime cache against the build's pinned font manifest before canonical rendering.
- When the runtime cache is missing or corrupt but the factory pack remains valid, rebuild the complete cache into temporary storage, verify hashes and a canonical renderer probe, then switch it atomically. Continue without prompting for reinstall and record the successful repair quietly in Diagnostics.
- Never repair bundled fonts from the network, operating-system fonts, an imported Celebration Package, or Parish Backup. Those sources may not silently become application dependencies.
- If the factory pack is missing or corrupt but the runtime cache remains fully verified, continue normal rendering, snapshot capture, Presenter Mode, and export. Report **Font recovery copy missing** as a non-blocking Diagnostics finding because output is sound but local redundancy has been lost.
- Offer **Repair Ordo** guidance for that degraded-redundancy state and direct the operator to run a trusted same-version Ordo installer from offline media. Keep application repair owned by the installer rather than teaching the running application to extract pieces from another installer.
- If both factory pack and runtime cache are unavailable or corrupt, no trusted local font bytes remain. Do not substitute system fonts for congregation output. Keep the operator shell usable with a system fallback, allow ordinary authoring/autosave, but mark exact slide previews unavailable and block new snapshot capture and Celebration Package export.
- In the both-copies-lost state, allow Presenter Mode only for a previously validated snapshot whose complete canonical rendered frames remain intact and pass integrity checks; scale those frames without recomposition. If current edits require a new snapshot, Presenter entry remains blocked.
- If ordinary same-version installer repair fails, fall back to a full trusted uninstall/reinstall that preserves Parish data under the separately adopted retention rules.
- Hide verification, reconstruction, atomic replacement, and structured failure behind one Font Integrity Module interface consumed by startup, Diagnostics, the Slide Renderer, Presenter preflight, and artifact export.
