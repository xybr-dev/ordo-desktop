---
id: 60
title: Unified Atkinson operator typography
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Unified Atkinson operator typography

- `Atkinson Hyperlegible Next` is Ordo's canonical operator-interface typeface as well as its canonical congregation-projection typeface.
- Operator chrome uses the locally bundled Regular, Medium, and SemiBold weights according to hierarchy and density. ExtraBold remains primarily a projection weight for lyrics and congregation responses rather than the default operator-interface weight.
- `Noto Sans` remains the bundled missing-glyph fallback for both operator and projection text. Ordo does not depend on a Windows- or Linux-installed system font, so the application retains its intended typography across supported platforms and offline transfers.
- Dense operator layouts must be visually qualified at both `1366x768` and `1920x1080`, because Atkinson's wider letterforms can consume more horizontal space than compact UI faces such as Source Sans 3 or Inter.
- Truncation, wrapping, and tabular-data treatments must be designed explicitly; the UI must not compensate for width pressure by silently shrinking ordinary interface text below its adopted legibility target.
- Source Sans 3 and Inter were evaluated as credible compact alternatives but were not adopted. The unified Atkinson family was preferred for legibility, multilingual suitability, and a coherent Ordo identity across preparation, presentation, and congregation output.
