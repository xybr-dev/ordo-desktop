---
id: 312
title: Allow a second audited unsafe module for monitor identity
status: accepted
date: 2026-11-07
recorded: 2026-11-07
tags: [platform, display]
supersedes: []
superseded_by: null
amends: [269]
amended_by: []
sources: [crates/platform/src/display/mod.rs, vendor/gpui-pre-windows/src/display.rs]
---

# Allow a second audited unsafe module for monitor identity

## Context

Decision [272](0272-space-resumes-output-after-a-disconnect.md) lets `Space` resume output when
**the same recognized congregation display** returns after a disconnect, and refuses any other
display. Recognition (`recognize()` in `crates/platform/src/display/mod.rs`) requires the resolution
to match and any saved `name` to match.

On Windows the `name` is always `None`. GPUI exposes no monitor name. Its display uuid is
`Uuid::new_v5(NAMESPACE_DNS, szDevice)` of the GDI device name `\\.\DISPLAYn`
(`vendor/gpui-pre-windows/src/display.rs`), so it identifies a port, not a monitor. "The same
display" therefore means "the same resolution": a different 1920×1080 projector or TV on the same
port would pass the Decision 272 guard.

The monitor's identity is available from the Windows display-configuration API: `QueryDisplayConfig`,
then `DisplayConfigGetDeviceInfo` per path. These calls need `unsafe`, and Decision
[269](0269-allow-one-audited-unsafe-module-for-process-memory-record.md) allows exactly one opted-out
module and says widening that needs a new decision. Scope, blast radius and risks are in issue #81.

## Decision

- `crates/platform/src/display/monitor_identity.rs` is the second, and last, module allowed to opt
  out of the workspace `unsafe_code` lint, beside `crates/platform/src/process_memory.rs`.
  `cargo xtask verify` allows exactly these two files and fails on any other opt-out.
- Its `unsafe` is limited to:
  - `GetDisplayConfigBufferSizes`, `QueryDisplayConfig (QDC_ONLY_ACTIVE_PATHS)` and
    `DisplayConfigGetDeviceInfo`;
  - fixed-size structs whose `size` and `type` header fields Ordo sets;
  - buffers sized from `GetDisplayConfigBufferSizes`, with one retry on
    `ERROR_INSUFFICIENT_BUFFER`.

  No pointer is retained or shared across threads. Off Windows the module compiles to a safe stub.
- **Matching to GPUI.** Each path's source name (`viewGdiDeviceName`) correlates with GPUI's display
  through the same uuid GPUI derives from it, so the match is exact rather than heuristic.
- **The name.** `DisplayEvidence::name` carries the friendly name with the EDID maker and product
  codes, e.g. `EPSON PJ (EPS/0x0123)`.
- **Missing or unreadable names.** A blank or generic name ("Generic PnP Monitor", "Generic Non-PnP
  Monitor"), an unreadable EDID, or a failed read counts as **no name**, and recognition falls back to
  the resolution rule. A failed read never panics.
- **When it is read.** The identity is read when output is enabled, on an explicit refresh and on
  display-change events (Decision 308). It is never polled.
- **Dependencies.** `windows-sys =0.61.2` gains only the `Win32_Devices_Display` feature. No new
  crate is added.

## Alternatives considered

- **Expose the name through the vendored GPUI patch.** `PlatformDisplay` has no name field, so this
  would mean patching `gpui-pre` itself. Decision 308 marks that as the point to reconsider forking.
- **A third-party crate, or PowerShell/WMI.** It is still `unsafe` underneath, and it adds
  supply-chain surface or a spawned, slow process on a safety path.
- **Do nothing.** A different display with the same resolution passes the Decision 272 guard.

## Consequences

- **The failure mode is a false refusal.** If Windows reports a different name for the same projector
  across connections (an extender or switch rewriting the EDID), `Space` is refused, and the operator
  recovers through Select display. The generic-name rule reduces this, and T1.6's `[MANUAL]` row checks
  it on the church cabling.
- **Two projectors of the same model** are still indistinguishable. That needs the EDID serial number,
  which is reachable only through more `unsafe` (SetupAPI/registry) or WMI.
- Any further `unsafe` module needs another decision.
