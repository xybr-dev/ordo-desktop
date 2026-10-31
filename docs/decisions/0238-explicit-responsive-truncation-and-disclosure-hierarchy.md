---
id: 238
title: Explicit responsive truncation and disclosure hierarchy
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

# Explicit responsive truncation and disclosure hierarchy

- Never truncate or replace with icon-only ambiguity the text for **Live**, **Freeze**, **Blackout**, congregation-output health, active warnings/errors, readiness counts, scheduled times, or the contextual primary action. Reflow these labels or adapt surrounding lower-priority content first.
- Let the current workspace or entity title wrap to at most two lines in its owning header. In narrow list, outline, and Ordo-rail rows, keep titles to one ellipsized line; allow Cue captions outside their canonical frame up to two lines.
- Reveal a visually truncated value in full on pointer hover and keyboard focus through the shared tooltip behavior. Its accessible name or associated text always contains the full value, never the rendered ellipsis.
- Collapse older breadcrumb ancestors into an accessible overflow menu before shortening the current entity. Keep the immediate navigation context and current entity visible.
- Remove or move secondary metadata into its owning Inspector/detail surface before compressing primary identity, state, or action text. Where compact formatting is necessary, use an intentional shorter equivalent rather than cutting characters unpredictably.
- Never ellipsize user-authored lyrics, readings, congregation responses, announcements, notes, or other editable prose. Wrap them or provide local scrolling within the bounded owning surface. Canonical Slide text remains governed exclusively by reviewed pagination and the Canonical Slide Renderer, never by operator-interface truncation.
- Prohibit marquee/auto-scrolling labels, hover-only access to essential meaning, and font shrinking below the design-system minimum as overflow remedies.
