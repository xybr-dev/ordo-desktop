---
id: 205
title: Deliberate sanitized Diagnostic-report export
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Deliberate sanitized Diagnostic-report export

- Add `diagnostic-report` as an explicitly supported `.ordo` artifact kind. **Export diagnostic report** creates it locally for deliberate file or removable-drive transfer; Ordo never uploads or sends it automatically and requires neither internet access nor an account.
- Include the normalized diagnostic summary and check results, application/operating-system/hardware environment, sanitized recent Ordo logs, error codes, timestamps, and anonymous entity identifiers needed to correlate a finding without exposing the entity's content.
- Exclude parish name, Song titles, lyrics, readings, notes, media, usernames, hardware serial numbers, complete filesystem paths, and file bodies. Render necessary paths with generic roots such as `[Ordo data]/assets/...` rather than leaking a user's profile or volume details.
- Provide **Copy summary** for a short operator-readable text summary that follows the same privacy boundary. It is a local clipboard action, not a network-sharing flow.
- Route creation through the adopted Artifact Container Module and obtain all diagnostic content through the Diagnostics Module's normalized result. The report exporter owns packaging and publication but must not independently recollect unsanitized runtime or parish data.
- Apply the universal Ordo-file safety and integrity contract to the report: `manifest.json`, declared kind and schema version, structured JSON payload, optional sanitized log assets, bounded extraction, checksums, and validation. Opening a Diagnostic report is read-only and can never mutate parish state.
