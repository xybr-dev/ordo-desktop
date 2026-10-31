---
id: 18
title: Lyric pagination
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: [255]
sources: []
---

# Lyric pagination

> **Superseded in part by Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md).** Its canonical semantic layout and
> authored-casing rules govern Lyrics; the pagination limits and safety rules
> below remain in force. Exact projected type sizes are provisional pending the
> Canonical Slide Layout Gallery.

- Newly encoded or imported lyrics begin with automatically proposed slide boundaries.
- The default lyric body size is `72` canonical canvas units, approximately equivalent to the existing decks' 54-point PowerPoint text.
- Automatic pagination targets two logical lyric phrases per slide and permits at most four rendered lyric lines.
- Intentional lyric lines, musical phrasing, punctuation, syllable division, and author-supplied phrase boundaries are preserved when proposing slide divisions.
- Verse, chorus, refrain, bridge, and similar section labels use a separately reserved label area and do not consume the four-line lyric budget.
- When a proposed slide would overflow, Ordo splits it at a phrase or intentional line boundary before reducing type size.
- Type may be reduced only as far as `64` canonical canvas units, approximately equivalent to 48-point PowerPoint text.
- Content that still does not fit is flagged for human review. Ordo never silently clips, squeezes, or shrinks it below the minimum.
- Automatic pagination is a draft convenience, not authoritative published structure.
- An operator reviews the generated slide sequence and approves it before the arrangement is treated as presentation-ready.
- Manual slide-boundary corrections take precedence over the automatic proposal and are stored with the arrangement.
- Changing reviewed lyrics, intentional line breaks, arrangement order, or another input that affects rendered pagination marks the affected arrangement **Needs Review** before a new Presentation snapshot is captured.
- Changing a theme marks affected slides for validation and requires review when overflow or another output problem is detected.
- Previously exported Celebration Packages remain unchanged because they contain immutable rendered snapshots.
- Pagination is approved within one independent Arrangement entry. Ordo does not maintain different slide-grouping maps that share the same cross-arrangement lyric lines.
- A materially different choir or seasonal form is encoded as a separate Arrangement entry and tagged for its applicable liturgical seasons.
