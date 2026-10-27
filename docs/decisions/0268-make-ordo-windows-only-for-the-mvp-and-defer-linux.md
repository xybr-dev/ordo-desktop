---
id: 268
title: Make Ordo Windows-only for the MVP and defer Linux
status: accepted
date: 2026-10-09
recorded: 2026-10-09
tags: [mass-plan, platform]
supersedes: [32]
superseded_by: null
amends: [2, 17, 28, 33, 234, 252]
amended_by: []
sources: []
---

# Make Ordo Windows-only for the MVP and defer Linux

- The owner dropped Linux on 2026-10-09. Development and preparation now
  happen on Windows (Windows 11 has become fast enough), and the owner does
  not expect to use Linux in the foreseeable future.
- MVP scope: preparation runs on a connected Windows machine; presentation
  runs on the air-gapped Windows 10 22H2 church PC. Both use the same Windows
  x64 installer. Linux preparation, the Linux Flatpak, and the Linux
  diagnostic ZIP are deferred post-MVP.
- Every `[NATIVE-AUTO]` and `[MANUAL]` gate names Windows only. The Linux
  checks this removes are not dropped silently: their original wording is
  kept, ungated, in the retired `MILESTONES.md` T6.2. That covers T0.4's
  Linux canonical-invariance and packaged-Linux legibility checks
  (`T0.4-MANUAL-GATE.md` script B, now retired), T0.6's Linux
  minimum-profile measurement, T0.10's supported-Linux sign-off, and T6.2's
  own Flatpak/ZIP criteria.
- CI: the `ubuntu-24.04` verify job is replaced by a `windows-2022` job that
  runs `cargo xtask verify`. The Windows shaping-probe and
  canonical-invariance jobs stay mandatory.
- Supersedes in part Decisions [2](0002-platform-and-displays.md), [17](0017-local-storage-and-portable-transfer.md), [28](0028-desktop-shell-supported-hardware-and-performance-budgets.md), [33](0033-source-hosting-and-cross-platform-builds.md), [234](0234-operator-only-interface-scaling-isolated-from-canonical.md), and [252](0252-replace-electron-with-a-clean-slate-rust-and-gpui.md), and Decision [32](0032-linux-installation-scope.md)
  in full; each carries a forward notice.
- This decision changes scope only. Golden frames, serialized canonical
  layouts, and bundled-font manifests are unchanged. If Linux is revived, its
  invariance runs must reproduce the Windows artifacts, and a mismatch
  reopens Decision [267](0267-freeze-the-t0-4-projected-type-levels-and-approve-the.md)'s evidence rather than adjusting the artifacts.
