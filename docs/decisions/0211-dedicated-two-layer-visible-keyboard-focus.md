---
id: 211
title: Dedicated two-layer visible keyboard focus
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

# Dedicated two-layer visible keyboard focus

- Show Ordo's custom keyboard-focus treatment through `:focus-visible` so keyboard navigation is unmistakable without leaving persistent rings after ordinary pointer activation.
- Use a consistent two-layer focus treatment in both appearances: a `2px` separation ring matching the immediate surface followed by a `2px` blue outer ring. Adopt `#2F6FA3` as `focus-preparation` and `#8FCBFF` as `focus-presenter`; keep the geometry identical while adapting contrast to the warm-paper and graphite surfaces.
- Keep focus independent from selection and operational state. Selection remains a forest-green tinted fill; focus remains the blue outer outline, and both may coexist on the same row, Cue, or control. Never reuse Live green, Freeze/warning amber, Blackout, danger red, or liturgical colors as focus.
- Apply the treatment to the outer wrapper of canonical slide thumbnails and frames. The outline may not enter, resize, crop, or otherwise alter the canonical `16:9` composition.
- Never suppress the browser or operating system's native outline unless the complete Ordo equivalent is present. In Windows high-contrast and other forced-colors environments, allow the system highlight treatment to take precedence.
- Implement focus centrally through the Ordo design-system Module and source-owned primitive seam rather than styling each workspace independently. Qualify the result in dense lists, menus, dialogs, nested slide-overs, editor rows, the Cue timeline, Presenter controls, both supported desktop sizes, and increased text scaling.
