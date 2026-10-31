---
id: 193
title: "Universal `.ordo` exchange container"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Universal `.ordo` exchange container

- Use `.ordo` as the single native extension for Ordo-compatible portable artifacts, including Celebration Packages, Parish Backups, Song imports, and Liturgy imports. Determine the artifact's meaning from its verified internal manifest rather than its filename or extension alone.
- Define an **Ordo file** as a standard ZIP container with a required root `manifest.json`, a structured JSON payload, and an optional `assets/` tree. JSON remains the readable serialization format; do not invent a proprietary binary serialization, obfuscation layer, or encryption merely to make the file appear native.
- Require the manifest to identify the Ordo container format, artifact kind, schema version, integrity metadata, and the payload/assets it owns. Initial artifact kinds include `celebration-package`, `parish-backup`, `song-import`, and `liturgy-import`.
- Treat `.ordo` as an exchange-container claim, not a trust claim. AI tools may create Song and Liturgy import files, so every artifact kind receives path-safety checks, extraction limits, schema validation, integrity verification, and kind-specific review before mutation.
- Publish the relevant JSON Schemas and packaging contract so AI agents and other offline tooling can create valid Song and Liturgy `.ordo` files with ordinary ZIP and JSON tooling.
- Centralize inspection, validation routing, safe extraction, manifest/version handling, and creation behind one deep **Artifact Container Module**. Import screens consume its small typed result interface instead of implementing ZIP, schema, and safety rules independently.
- Associate `.ordo` with Ordo on supported desktops and route double-clicked files to the review flow declared by their verified artifact kind. Keep emergency decks as `.pptx`; plain authoring/debug fixtures may remain JSON without becoming end-user import artifacts.
