---
id: 216
title: Four-pixel spacing scale with resolution-aware density
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Four-pixel spacing scale with resolution-aware density

- Use a `4px` base spacing scale with named values `4`, `8`, `12`, `16`, `24`, and `32px`. Use `8px` between closely related elements, `16px` between groups, and `24px` between major sections; avoid arbitrary one-off gaps.
- Make default controls and ordinary list rows `36px` high. Allow compact icon controls at `32x32px`, and keep high-stakes Presenter controls at least `40px` high.
- Use `16px` workspace gutters at the compact `1366x768` qualification size and `24px` gutters at `1920x1080`. Overlays continue using their adopted viewport-bound exterior gutters and internal scrolling behavior.
- Use additional desktop area for wider canonical previews, more slide columns, and more visible context rather than proportionally scaling the entire interface. Preserve the established responsive disclosure and optional-rail behavior.
- Let increased interface text scaling grow rows and reflow or disclose secondary information. Never clip labels, overlap controls, or shrink essential text merely to preserve nominal row height.
- Do not provide a user-facing Compact/Comfortable density preference in the MVP. One qualified adaptive density system is sufficient.
