---
id: 32
title: Linux installation scope
status: superseded
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: 268
amends: []
amended_by: []
sources: []
---

# Linux installation scope

> **Superseded in full by Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md).** No Linux package ships with the MVP.

- Ship an x64 Flatpak as the supported installable Linux MVP release.
- The Flatpak is the single first-class Linux installation format across CachyOS and other supported Linux distributions; Ordo does not initially maintain distribution-specific installation packages.
- Also produce an x64 ZIP archive as a portable development, diagnostic, and recovery fallback.
- The ZIP fallback does not provide normal desktop installation or integration and is not the preferred end-user installation path.
- Defer `.deb`, `.rpm`, AppImage, and native Arch packages until actual user demand justifies their additional build and acceptance-test surface.
- Keep Linux packaging behind the release-packaging Seam so package format does not affect Ordo's domain Modules or portable data formats.
