---
id: 223
title: Strict Invalid versus Needs-decision import states
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

# Strict Invalid versus Needs-decision import states

- Use **Invalid** only when an Ordo file or contained document fails its contract, archive/path safety, checksum or integrity verification, required dependency closure, or supported schema-version check. Invalid content can never be force-imported or converted into an operator override.
- Keep every failed file visible in its review session with the exact human-readable reason and actions to **Choose another file**, **View technical details**, or **Remove from review**. Do not dismiss the workflow, reduce the failure to a toast, or mutate Parish data.
- Distinguish a valid newer schema from corruption. State that the installed Ordo version cannot read it and may need to be updated; preserve its technical version evidence without attempting a best-effort import.
- In multi-document Song or Liturgy intake, mark an invalid document **Excluded** and allow independently valid documents to continue through review. If any Arrangement or internal reference within one Song import document is invalid, exclude that complete document rather than salvaging fragments.
- Treat a Celebration Package and Parish Backup as indivisible verified artifacts. Failure of any required manifest entry, dependency, asset, snapshot, checksum, or stored-data check blocks the entire artifact before reconciliation, restoration, or installation.
- Reserve **Needs decision** for structurally valid content whose identity or reconciliation outcome needs an operator choice. It blocks the atomic commit until resolved but must not be styled or described as corrupt.
- Perform no database, managed-asset, Package-history, or Parish mutation before the accepted valid set commits atomically. Removing or replacing an invalid review input changes only temporary staging.
