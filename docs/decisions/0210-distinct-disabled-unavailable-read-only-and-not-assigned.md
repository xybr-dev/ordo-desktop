---
id: 210
title: "Distinct disabled, unavailable, read-only, and not-assigned semantics"
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

# Distinct disabled, unavailable, read-only, and not-assigned semantics

- Use **Disabled** only for an action or input that exists but cannot currently execute. Keep its styling neutrally subdued; do not use warning or danger color merely because the control is disabled.
- Use an ordinary native disabled control when its reason is obvious from local context, such as **Save** when nothing changed. When the reason is operationally consequential, such as a blocked Parish restore, keep the control keyboard-focusable, expose its unavailable state through AccessKit, suppress activation, and associate it with a persistent nearby explanation that assistive technology can announce.
- Never rely on a hover-only tooltip to explain a consequential disabled action. A pointer tooltip may repeat the reason but cannot be its sole presentation.
- Use **Unavailable** for content or a capability that cannot currently perform its domain purpose, such as a Cue with a missing dependency or a disconnected saved display. Keep it visible and sufficiently legible, label it **Unavailable** with an icon and short reason, preserve inspection and corrective navigation, and prevent actions that could expose or commit the invalid content.
- Use **Read-only** for intentionally inspectable content that cannot be edited in the current context. Prefer omitting editing controls and stating the read-only context over rendering an entire editable form as disabled.
- Treat **Not assigned** as a valid configuration value, particularly for congregation-display selection. It is neither an error nor the same state as a previously assigned display becoming Unavailable.
- Keep these meanings consistent in Ordo Modules and primitive adapters. A component-library `disabled` prop must not erase Ordo's distinction between action state, content capability, inspection mode, and valid empty assignment.
