---
id: 190
title: Reuse a content-equivalent Package version on export
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

# Reuse a content-equivalent Package version on export

- Allow Celebration Package export even when the Celebration's current export scope is content-equivalent to the latest retained Package version. Do not make the absence of changes an error or disable portable copying.
- Determine equivalence from the Celebration, all of its current Mass plans, and the complete dependencies reachable from those plans. Unrelated parish-library edits do not invalidate reuse.
- When equivalent, disclose **No changes since Package version …** and copy the exact retained immutable artifact to the chosen destination. Preserve its Package-version UUID and fingerprint rather than manufacturing a new version or regenerating equivalent files.
- Let the receiving installation recognize that exact version as **Already imported** and perform no duplicate package, snapshot, asset, revision, or library mutation.
- Create a new Package-version UUID and artifact fingerprint only when relevant exported content changed. Apply the existing captured-or-reused Presentation-snapshot rule independently to each included Mass plan.
