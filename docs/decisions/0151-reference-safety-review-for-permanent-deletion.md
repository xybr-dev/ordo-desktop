---
id: 151
title: Reference-safety review for permanent deletion
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Reference-safety review for permanent deletion

- Open one viewport-bounded reference-safety dialog when **Delete permanently** is invoked for an archived Song or Arrangement.
- If deletion is blocked, group and identify remaining child Arrangements, Mass plans, Presentation snapshots, and other durable references. Link to editable blockers where practical and do not expose an enabled deletion commit.
- If deletion is safe, state exactly which entity content and owned Edit revisions will be erased, then offer **Cancel** and **Delete permanently**. The overflow selection plus this explicit dialog provides sufficient deliberation; do not require retyping the title.
- Treat the entity's own revisions as part of the entity being deleted rather than as self-blocking external references. Disclose their loss in the safe-state summary.
- Make permanent deletion non-undoable. Keep Archive and Restore as the ordinary recoverable lifecycle path.
