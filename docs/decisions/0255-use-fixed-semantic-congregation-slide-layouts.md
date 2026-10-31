---
id: 255
title: Use fixed semantic congregation-slide layouts
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, rendering, ui]
supersedes: []
superseded_by: null
amends: [18, 19, 46, 57, 100]
amended_by: [262, 263, 264, 265]
sources: []
---

# Use fixed semantic congregation-slide layouts

> **Superseded in part by Decisions [262](0262-render-reading-identification-in-sentence-case-at-the.md), [263](0263-render-reading-passage-at-a-readingpassage-only-88px.md), [264](0264-render-response-at-a-response-only-113px-candidate-centered.md), and [265](0265-content-size-psalm-and-response-bodies-and-keep-authored.md).** [262](0262-render-reading-identification-in-sentence-case-at-the.md) replaces the Reading-identification rule, [263](0263-render-reading-passage-at-a-readingpassage-only-88px.md) the shared `70px` passage size for ReadingPassage only, [264](0264-render-response-at-a-response-only-113px-candidate-centered.md) the Response size and vertical placement, and [265](0265-content-size-psalm-and-response-bodies-and-keep-authored.md) the display-size Psalm refrain, uppercase Song title, and left-aligned Lyrics body. The rest remains authoritative. (Notice added when the ledger was split; the decisions said so in their own text.)

- This supersedes Decision [19](0019-projector-safe-area.md)'s default safe-area dimensions, Decisions [46](0046-projector-calibration-and-overscan-compensation.md)/[57](0057-physical-church-system-release-qualification.md)'s
  corresponding calibration/qualification guides, and the relevant
  layout/background limits in Decisions [18](0018-lyric-pagination.md) and [100](0100-theme-appearance-and-mvp-background-model.md); their unaffected safety and
  inheritance rules remain.
- Compose every Slide in renderer-owned header, body, and footer zones on the
  canonical `1920x1080` canvas. Do not support freely draggable elements in the
  MVP. Use default safe-area insets of `120` horizontal and `80` vertical
  canonical units; Themes may increase but not reduce them. An absent header or
  footer collapses so the body receives the space.
- Lyrics use an optional top-left section label and authored-case, left-aligned
  text optically centered in the remaining region. The label defaults on,
  repeats on every generated Slide, and is toggled once per Song item.
- Response Slides render an optional repeated authored header and an emphasized
  uppercase congregation response. A language-specific Mass template owns the
  default copied into its plan, and an item may inherit, hide, or override it;
  `TUBAG:` is the Cebuano template default, not a renderer literal. Store
  authored casing unchanged. Response sequences contain ordered optional
  private minister text plus required projected congregation responses.
- Reading identification uses a large uppercase proclamation title with its
  reference at bottom-left and no redundant First/Second/Gospel output label.
  Subsequent Scripture Slides preserve authored casing, use smaller text, retain
  the reference on every Slide, paginate automatically, and honor blank lines as
  hard boundaries.
- Ordinary Gospel templates may order an optional opening exchange,
  identification, congregation acclamation, optional Scripture, and closing
  exchange. Language-specific templates own the wording and optional blocks. A
  Good Friday preset may omit the opening greeting and pre-passage acclamation
  while retaining the closing exchange.
- Responsorial Psalms use the same template-default plus plan/item override
  rule; `SALMO RESPONSORYO` is the Cebuano template default. They project the
  refrain before the first cantor verse and after every verse (`n + 1` linked
  occurrences). Cantor verses are private by default and use Wait Cues that
  leave Program unchanged; optional projection turns them into ordinary
  Scripture-style Slides.
- Song identification is a bottom-left block with a large uppercase title,
  smaller composer/source, and optional Arrangement/Choir credit. The automatic
  selectable Celebration-title Cue precedes the first Mass section, defaults
  on, doubles as the idle Cue, uses an uppercase optically centered title, and
  anchors its localized date bottom-left.
- Celebration-title appearance may inherit the Mass Theme, select another
  reusable Theme, or replace only the background with one Mass-local image.
  Keep the resolved Theme's other typography/layout rules and require cover,
  focal-position, overlay, contrast, and fallback-color handling.
- General Presentations have only constrained Text and Image modes: optional
  header/center-left body/footer, or one contained foreground image with
  optional header/caption. Theme backgrounds remain separate.
- Validate actual canonical geometry, never character count. Lyrics target two
  phrases and no more than four rendered lines; split before shrinking and warn
  below the approved minimum. Song and Celebration titles allow at most three
  lines before readiness warning. Exact numeric type levels remain provisional
  until the retained Canonical Slide Layout Gallery is visually qualified.
