---
id: 206
title: Latest-result plus explicit full Diagnostic scan lifecycle
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

# Latest-result plus explicit full Diagnostic scan lifecycle

- Show the latest successfully completed full Diagnostic result immediately, together with the exact completion time, rather than treating stale or partial work as current health evidence.
- On page entry, refresh only inexpensive live facts such as application identity, free storage, and current display connection. Do not automatically run database integrity, complete asset hashing, font validation, or renderer qualification merely because Settings opened or Ordo started.
- Make **Run full diagnostics** an explicit, asynchronous operation with visible overall and per-section progress. It must leave the rest of the operator interface responsive and must never run automatically during Presenter Mode.
- Allow cancellation without changing parish data or erasing the previous completed result. A cancelled, interrupted, or failed run is labeled as an incomplete attempt and never replaces the latest completed overall **Healthy** or **Needs attention** assessment.
- Publish the new overall assessment and persist its normalized result only after the complete requested scan finishes. Individual in-progress checks may show provisional progress but must not masquerade as the current completed result.
- Keep the scan lifecycle behind the Diagnostics Module's interface so the Settings page observes state and issues start/cancel commands without orchestrating database, asset, font, display, or renderer checks itself.
