---
id: 22
title: Guaranteed fallback Theme
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Guaranteed fallback Theme

- Ordo always retains a built-in fallback Theme so missing or damaged custom assets cannot make a Mass unpresentable.
- The fallback uses a pure black background, white heavy sans-serif text, and centered lyrics and responses.
- It has no decorative image, texture, shadow, or animation.
- It uses a bundled open-license font rather than depending on Arial Black or another operating-system font being installed.
- Custom Themes receive automatic contrast validation warnings.
- `Atkinson Hyperlegible Next` is the canonical bundled projection font, using ExtraBold for lyrics/responses and SemiBold for titles, credits, and labels.
- `Noto Sans` is the bundled missing-glyph fallback.
- Arial Black is retired from Ordo's font stack. It is neither the default nor a fallback because relying on an operating-system font would undermine deterministic Linux/Windows rendering and portable Celebration Packages.
- The earlier comparison's apparent Arial Black rendering on Linux was a system substitution, not genuine Arial Black; this reinforced the requirement to bundle the exact selected font files.
- An actual church-projector test remains an acceptance check for exact sizing, spacing, washout resistance, and pagination metrics, but it does not leave the font-family choice provisional.
