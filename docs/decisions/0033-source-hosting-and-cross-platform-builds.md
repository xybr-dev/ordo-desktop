---
id: 33
title: Source hosting and cross-platform builds
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: [252, 268]
sources: []
---

# Source hosting and cross-platform builds

> **Superseded in part by Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md).** CI and releases are Windows-only; the Linux runner, Flatpak, and ZIP outcomes no longer apply.

> **Superseded in part by Decision [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md).** GitHub, native Windows/Linux CI, draft
> releases, checksums, and data isolation remain. The npm/Node/Electron
> toolchain requirements do not.

- Keep the Ordo MVP source in a private GitHub repository.
- Use GitHub Actions as the cross-platform continuous-integration and release-build environment.
- Build and acceptance-test the x64 Windows installer on a Windows runner rather than relying on Linux-to-Windows cross-compilation.
- Build and acceptance-test the x64 Linux Flatpak and ZIP on a Linux runner.
- Ordinary commits and pull requests run verification without publishing a release; a version tag initiates the versioned release-build workflow.
- A successful version-tag workflow creates or updates a draft GitHub Release rather than immediately publishing a final release.
- The draft collects all platform artifacts, checksums, and release notes. The product owner explicitly reviews and publishes it; publication is not an automatic consequence of a passing build.
- Draft publication is blocked unless the actual packaged Windows and Linux applications pass automated smoke tests on their native CI runners; passing only source-level tests is insufficient.
- Packaged smoke tests cover application launch, creation or opening of temporary parish data, migration of a representative fixture database, rendering of known canonical slides, Celebration Package export-and-open, and Parish Backup creation and verification.
- Physical two-display validation remains a distinct release/deployment concern because hosted CI cannot establish that a particular church projector, display topology, or driver behaves correctly.
- Use npm as the package manager, commit `package-lock.json`, and use clean `npm ci` installs in CI.
- Pin the Node.js major release and exact Electron and Electron Forge versions used for a release so Linux development and native CI builds use a qualified toolchain rather than silently drifting.
- Release output includes cryptographic checksums and retained downloadable artifacts so the Windows installer can be downloaded and verified on the connected Linux preparation machine before removable-media transfer.
- Parish databases, content libraries, Celebration Packages, Parish Backups, and other parish data never enter the source-hosting or CI environment.
- Whether Ordo's source becomes public and which open-source license might apply are product-governance decisions explicitly deferred until after the MVP. The private MVP repository must not make later open-sourcing unnecessarily difficult.
