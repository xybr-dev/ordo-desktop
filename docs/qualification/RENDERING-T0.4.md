# Canonical rendering dependency qualification for T0.4

## Decision

Adopt `rustybuzz` `0.20.1`, `ttf-parser` `0.25.1`, `swash` `0.2.10`, and
`tiny-skia` `0.11.4` as direct `rendering` dependencies: a shaper plus a glyph
rasterizer that Ordo drives explicitly, behind Ordo-owned itemization,
wrapping, fallback, and composition (PR 4, PR 7). No layout engine is adopted.

| T0.4 need | Pinned crate | License | Decision and boundary |
| --- | --- | --- | --- |
| Shaping | `rustybuzz 0.20.1` | MIT | Adopt the HarfBuzz shaping algorithm only. Ordo supplies the buffer, script, direction, features, and itemization; the crate orders, substitutes, and positions glyphs and nothing else. |
| Font parsing | `ttf-parser 0.25.1` | MIT OR Apache-2.0 | Adopt zero-allocation table parsing behind the shaper and rasterizer. Already a transitive dependency of the adopted shaper; the direct edge states the reliance instead of inheriting it. |
| Glyph rasterization | `swash 0.2.10` | Apache-2.0 OR MIT | Adopt outline introspection and CPU glyph rasterization. Ordo chooses the scale, transform, and which glyph of which bundled face rasterizes; the crate fills coverage and nothing else. |
| Frame composition and PNG encoding | `tiny-skia 0.11.4` | BSD-3-Clause | Adopt the software 2D rasterizer and its `png-format` encoder. Ordo owns every coordinate, clip, blend order, and the 1920x1080 logical space; the crate executes pixels. PNG bytes for golden fixtures and operator handoff come through this edge, so no direct `png` dependency is adopted. |

All four licenses are inside `deny.toml`'s allow list. All four versions were
already in `Cargo.lock` transitively through the pinned GPUI graph; the
lockfile change for this qualification is edge-only (`rendering` gains four
named dependencies, zero new packages, zero version changes), confirmed by
`git diff Cargo.lock`.

## Rejected

| Candidate | Version in lock | Decision |
| --- | --- | --- |
| `cosmic-text` | `0.19.0` via `gpui_wgpu` | Reject. It owns line breaking, BiDi resolution, fallback, and its shaping cache, which is exactly the control AC 5 requires Ordo to hold. Its `fontdb` backend additionally scans installed system fonts through per-OS backends (DirectWrite on Windows, fontconfig on Unix, Core Text on macOS; `cfg(target_os)` throughout `fontdb`), which contradicts AC 3's bundled-font isolation. Adopting it would also couple the canonical path to GPUI's own text behavior, contradicting AC 5 structurally. |
| `image` | `0.25.10` via `gpui` | Reject. A full codec surface (BMP, GIF, JPEG, WebP, and more) for a renderer that encodes PNG only. `tiny-skia`'s `png 0.17` edge already covers encoding; the operator-side image loading that pulls `image` through `gpui` stays on the operator side. |
| direct `png` | `0.18.1` via `image`, `0.17.16` via `tiny-skia` | Defer. Nothing in `rendering` names the encoder directly while `tiny-skia` exposes it. If a later PR needs a decoder or encoder option `tiny-skia` does not forward, that PR re-runs this qualification for the direct edge. |

## Determinism evidence

Source inspection of the pinned revisions (registry sources, September 2026):

- `rustybuzz 0.20.1`: zero `target_os`/`cfg(windows)`/`target.*` gating across
  all 74 source files. Shaping is integer font-unit arithmetic over the input
  bytes; no clock, locale, or OS text stack participates.
- `swash 0.2.10`: zero OS gating across all 61 source files. Default features
  `std, scale, render` are pure computation.
- `tiny-skia 0.11.4`: zero OS gating across all 44 source files. Default
  features `std, simd, png-format`; `simd` selects compile-time SIMD paths that
  are identical for a given target feature set on both packaged targets.
- `ttf-parser 0.25.1`: a pure parser; default features include
  `opentype-layout`, `apple-layout`, and `variable-fonts`, which the probe
  exercises through the variable Atkinson face at its default instance.

Scope note, per AC 6: byte-for-byte invariance is claimed for serialized
*logical* output (glyph ids, advances, offsets, wrapping, pagination inputs,
safe area, element bounds). Raster sharpness is qualified separately per
platform in PR 8; the stack choice keeps that qualification to sharpness
rather than composition.

## Removal seam

All four crates are confined to `rendering` internals. No type from any of them
may cross the Renderer Interface PR 3 defines: frames leave `rendering` as
finished pixels plus logical size, exactly as `ui`'s viewport wrapper requires.
Replacing the shaper changes only the shaping module; replacing the rasterizer
changes only the raster module; replacing the compositor changes only the
composition module. `cargo xtask verify` enforces the seam from the outside
(see Policy below). Any revision change, or any fifth direct dependency,
invalidates this qualification.

## Invariance probe

`crates/rendering/tests/shaping_probe.rs` shapes one mixed-script buffer
(Cebuano/English/Tagalog Latin, digits, punctuation, em-dash, one Han
character) with the adopted shaper at LTR/Latin over the bundled Atkinson
bytes, and pins the integer glyph ids, advances, offsets, and byte clusters in
`crates/rendering/tests/fixtures/shaping-probe.txt`. Script segmentation and
Noto fallback are PR 4 scope: the probe deliberately records the raw behavior,
including the Han character mapping to `.notdef` (glyph 0) until fallback
lands. The probe is evidence, not the renderer, and its font path follows the
faces to their PR 2 shared location. The fixture is LF-pinned in
`.gitattributes`: the test compares it against in-memory `writeln!` output, so
a CRLF checkout on Windows would fail the comparison without any shaping
drift.

Linux run, `rustc 1.96.0`, x86_64:

```text
sha256 d3976b55459555c19f617f636f2a5e41b78ea1dd95ebea6d7858504efa834b71  shaping-probe.txt
```

Windows Server 2022 CI run, `rustc 1.96.0`, x86_64: the
[`windows-shaping-probe` job](https://github.com/xybr-dev/ordo-desktop/actions/runs/34104283694/job/101685632531)
regenerated the fixture at commit `155c82f`, found no byte diff, and produced
the same `d3976b55459555c19f617f636f2a5e41b78ea1dd95ebea6d7858504efa834b71`
digest. PR 8 expands this early probe to the packaged canonical renderer; a
mismatch there re-opens this decision rather than adjusting the artifact.

## Packaged invariance (PR 8)

`tests/native-runtime/tests/canonical_invariance.rs` renders one
mixed-language Lyrics probe (Cebuano/English/Tagalog Latin, digits,
punctuation, em-dash, plus Greek alpha and peso sign resolved through the
bundled Noto fallback with real glyph ids — the faces `rendering`'s
`tests/fonts.rs` pins as fallback-carried) through `render_template` at the
default Theme, and pins the serialized logical frame plus its diagnostics in
`tests/native-runtime/golden/canonical-invariance.txt` (LF-pinned in
`.gitattributes`). The harness additionally asserts the serialization
contains no `id=0`, so a `.notdef` can never hide inside a passing pin. The
probe runs where the packaged binary runs: the `native-runtime` harness
builds the `ordo` closure, and the lockfile change is edge-only
(`native-runtime-tests` gains the `rendering` dev-edge for the pure gate,
zero new packages, zero version changes).

One test asserts the density and platform axes, and two live-runtime tests
assert the scale axes against real geometry; each failure names the axis
that moved. Every scale byte is composed by the worker-owned production
path: the runtime-owned `ordo-canonical` worker thread serves
`CanonicalCommand::Compose` requests carrying a live environment snapshot,
the live window scale, and content, and replies with the completed frame
plus its diagnostics — the real production payload PR 9's viewport
consumers take (and rasterize from). The boundary derives renderer inputs
through the `ordo::canonical` production seam and renders off the GPUI
thread. The tests snapshot live operator state through production plumbing
after each transition, receive the worker-composed payload, and serialize
it for the pin at the edge — so the gates and future consumers exercise
the same worker path. PR 9's viewport consumers compose real slide content
through this same worker.

| Axis | Values exercised | How the failure reads |
| --- | --- | --- |
| Output density | `100`–`200` in `25` steps | `canonical composition moved with output density N%` |
| Ordo Interface scale | live `100%`→`200%`→`100%` through the real scale actions; operator token geometry is read back per step and must move while the production-path bytes must not | `canonical composition moved with live operator scale N%` |
| System scale | window scale factors `1.0`–`2.0` applied to the packaged operator window through gpui's display-scale seam; the production-path bytes are captured on that same window per step | `canonical composition moved with system scale N%` |
| Packaged platform | committed golden compared on both CI targets | `packaged <os> serialization differs from ...` |

The renderer exposes no input for either scale (no parameter, no
environment read, no GPUI type), and the production boundary spends neither
value: a change that routes either scale into composition inputs edits that
boundary and fails naming the scale — verified by spending operator scale
on the safe-area inset and watching the gate fail at `live operator scale
200%`. Density additionally pins the raster extent (`100%` → `1920x1080`,
`200%` → `3840x2160`) to prove it reaches pixels only. The operator-scale
test is the isolation proof T0.3's PR 3 deferred for want of a render path.
The lockfile change stays edge-only (`ordo` gains the `rendering` edge the
crate-graph contract already permits, zero new packages, zero version
changes).

Linux run, `rustc 1.96.0`, x86_64 (candidate T0.4.10 levels, pending human
approval: display 90px / passage 70px / meta 50px, +25% from the PR 5
provisional 72/56/40, plus a ReadingPassage-only pending candidate at 88px/400
(70 * 1.25 = 87.5, rounded; 102px advance) with GeneralText staying at 70px
and operator/UI unchanged; the invariance probe now composes 4 body lines where
it composed 3, with identical glyph ids and fallback routing):

```text
sha256 5708fd350d7d6e6b01a0c3368d63f6459c269b320c697489f9d7243698bcd28a  canonical-invariance.txt
```

Windows Server 2022 CI enforces the same digest through the
`windows-canonical-invariance` job, which runs the full invariance suite —
density, platform, and both live scale tests — and prints the artifact hash;
a mismatch fails the test before hashing. Linux runs the same suite under
`cargo xtask verify`, which executes the whole workspace suite. The golden
regenerates only
with `ORDO_UPDATE_CANONICAL_INVARIANCE=1`, and only after review: a
platform mismatch re-opens this decision rather than adjusting the
artifact.

Sharpness disclaimer, per the plan: this gate claims serialized *logical*
composition only and makes no claim about raster pixels. Per-platform
raster sharpness is qualified separately in the PR 10 manual gate.

## Policy

`xtask` guards `rendering` the way `QUALIFIED_UI_DEPENDENCIES` guards `ui`:

- `rendering`'s normal dependencies must stay within
  `QUALIFIED_RENDERING_DEPENDENCIES` (`rustybuzz`, `swash`, `tiny-skia`,
  `ttf-parser`, plus the internal `domain` edge covered by the crate-graph
  contract... see note).
- No GPUI crate (`gpui`, `gpui_platform`, `gpui-base`) may appear in
  `rendering`'s normal or development dependencies; the existing
  `gpui-base`-stays-private-to-`ui` check is unchanged.

Note: the qualified list covers external crates only. Internal edges
(`rendering -> domain`) remain owned by the crate-graph contract in
`validate_dependency_edges`, the same split the `ui` policy uses.
