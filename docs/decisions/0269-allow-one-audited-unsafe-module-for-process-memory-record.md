---
id: 269
title: "Allow one audited `unsafe` module for process memory; record GPU memory as unavailable"
status: accepted
date: 2026-10-10
recorded: 2026-10-10
tags: [platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Allow one audited `unsafe` module for process memory; record GPU memory as unavailable

- T0.6 asks for "RSS/private memory". sysinfo 0.31.4 reads neither Windows
  `PrivateUsage` nor any OS peak counter, so private memory was missing and
  peaks were sampled every 250 ms, which can miss a short spike.
- `crates/platform/src/process_memory.rs` makes one `GetProcessMemoryInfo`
  call through `windows-sys` `=0.61.2` (the version already in the lockfile)
  and returns `PrivateUsage`, `PeakPagefileUsage`, and `PeakWorkingSetSize`.
  Off Windows it returns `None`. Peaks are now exact since process start.
- This is the workspace's only `unsafe` exception. The workspace lint is
  `unsafe_code = "deny"` rather than `forbid` so that this one module can
  carry `#[allow(unsafe_code)]`. `cargo xtask verify` fails if any other
  source file opts out. Widening the exception needs a new decision.
- Rejected: the `memory-stats` crate. It returns `PagefileUsage` (the same
  charge) but drops the peak fields, labels a non-private figure the same
  way on other platforms, adds a third `windows-sys` version, and has not
  been released since 2024-06-26.
- GPU memory is recorded as unavailable. The pinned GPUI does not expose its
  graphics device, so a DXGI `QueryVideoMemoryInfo` query would have to guess
  the adapter and could report the wrong card on a two-GPU machine without
  any sign. That meets "GPU allocations where the platform exposes them"
  for the pinned stack. Revisit when GPUI exposes its device.
