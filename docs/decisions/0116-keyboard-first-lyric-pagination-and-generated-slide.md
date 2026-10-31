---
id: 116
title: Keyboard-first lyric pagination and generated-slide navigation
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, rendering, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Keyboard-first lyric pagination and generated-slide navigation

- Encode one Arrangement section in a plain multiline lyric editor. A normal newline continues the current slide; one blank source line between lyric groups starts the next generated slide.
- Preserve ordinary text-editing semantics. Enter and Backspace are not branded or displayed as dedicated Ordo shortcuts: inserting the blank line creates a boundary, and deleting it naturally joins the neighboring slide groups.
- Keep the two GUI boundary actions immediately adjacent. Their labels are contextual and name the exact consequence, such as **Split after line 2** and **Join slide 2 with slide 1**, rather than showing ambiguous key badges.
- Disable the join action when the first generated slide is selected. Keep a quiet, persistent rule beside the editor: **A blank line starts a new slide.**
- Render generated slides as a compact, wrapping navigator of fixed-size `16:9` thumbnails. A lone thumbnail retains the same width and height it would have in a fuller row; it never stretches to fill the available column.
- Treat the Inspector as the single authoritative large preview. Selecting a navigator thumbnail selects its corresponding source range and drives the Inspector; editing the source regenerates the navigator and Inspector without introducing a separate Slides task tab.
