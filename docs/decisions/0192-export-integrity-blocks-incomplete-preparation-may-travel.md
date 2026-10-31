---
id: 192
title: Export integrity blocks; incomplete preparation may travel
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

# Export integrity blocks; incomplete preparation may travel

- Treat Celebration Package export as a portability operation, not an assertion that every included Mass plan is presentation-complete. Allow incomplete content to travel so preparation can continue on another installation.
- Show unfilled Song or Reading slots, pagination/readability concerns, absent optional attribution or source metadata, never-presented plans, and ordinary Mass readiness findings as per-plan warnings. Keep **Export package** available without an additional approval dialog or checkbox.
- Block export only when Ordo cannot produce a trustworthy self-contained artifact: the Celebration has no Mass plans; a required asset or bundled font is missing or corrupt; a required Presentation snapshot cannot be captured or validated; dependency closure is incomplete; durable identities or revision lineage are inconsistent; or the destination cannot be published safely.
- Summarize preflight as **Ready**, **Ready with warnings**, or **Cannot export**. Let actionable findings navigate to their owning content or expose a direct repair/retry action when navigation is not applicable.
- Re-run affected checks after a repair and before final publication. Do not weaken manifest, checksum, atomic-write, or post-write verification merely because the only visible content findings were warnings.
