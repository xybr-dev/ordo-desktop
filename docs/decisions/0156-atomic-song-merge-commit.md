---
id: 156
title: Atomic Song-merge commit
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Atomic Song-merge commit

- Treat the completed merge review as the confirmation boundary. Commit through one explicit **Merge songs** action without adding another dialog or requiring the operator to retype a title.
- Disable commit while any blocking identity conflict, such as contradictory composer or original-author metadata, remains unresolved. Likely duplicate-Arrangement findings are review warnings and do not block because Arrangements remain independent.
- Execute metadata resolutions, intact Arrangement reparenting, creation of the retiring-Song redirect, and recording of the dedicated merge-history event in one atomic storage transaction.
- If validation, storage, asset coordination, or any other part of the transaction fails, roll back the entire attempt and leave both Songs, all Arrangements, and all references unchanged.
- After success, navigate to the surviving Song detail and show a confirmation with **Undo merge**. Do not leave the operator on a retired identity or a stale review surface.
