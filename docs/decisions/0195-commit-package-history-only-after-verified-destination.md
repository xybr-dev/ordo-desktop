---
id: 195
title: Commit Package history only after verified destination publication
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

# Commit Package history only after verified destination publication

- Treat a newly changed Celebration Package export as a recoverable staged transaction. Build the candidate Package version in managed temporary staging, validate it locally, publish it to the selected destination, read it back and verify it, then commit that Package version to Package history as **Exported**.
- Do not expose a staged candidate as Current or as an ordinary Package-history version before destination verification succeeds. Cancellation, removable-drive disconnection, insufficient storage, write failure, or failed read-back verification leaves no new Package version in History.
- Preserve or restore any previous destination file according to the adopted safe-replacement rule. Never leave a partial `.ordo` under the final filename.
- Journal the narrow interval between successful destination verification and local History commit. After restart, compare the destination artifact fingerprint with the staged candidate and either finalize the matching History entry or discard the abandoned candidate without guessing.
- Clean abandoned staging immediately when safe or during startup recovery. Retain current parish edits and previously committed Package versions regardless of export failure.
- Never advance a sync anchor merely because an artifact was exported. Only a successful reviewed import proves that two installations share the imported Edit revision.
