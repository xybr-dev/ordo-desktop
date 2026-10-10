# Ordo Desktop

Ordo is an offline-first Catholic liturgical presenter for parish audiovisual
operators. This repository is the clean-slate Rust + GPUI production
implementation.

## Start here

- [`CONTEXT.md`](./CONTEXT.md) — canonical domain language.
- [`DESIGN.md`](./DESIGN.md) — visual and interaction contract.
- [`MVP-PLAN.md`](./MVP-PLAN.md) — strategic product and architecture baseline.
- [`MILESTONES.md`](./MILESTONES.md) — ordered implementation tasks and gates.
- [`LEARNING-ROADMAP.md`](./LEARNING-ROADMAP.md) — Rust/GPUI learning runway.
- [`docs/agent-development.md`](./docs/agent-development.md) — agent-assisted
  development strategy.

## Run the app

From the repository root:

```sh
cargo run --locked
```

This opens the current operator window. Close it to stop the app; the
congregation window is not implemented yet.

## Run the UI lab

`ui-lab` is the development-only scenario host. It is not part of a parish
install and is compiled only when its `lab` feature is enabled:

```sh
cargo run -p ui-lab --locked
```

Each key cycles, so the lab can be driven one-handed while watching the stage:

| Key | Cycles |
| --- | --- |
| `F2` | scenario |
| `F3` | viewport preset — 1366x768, 1920x1080 |
| `F4` | appearance — System, Light, Dark |
| `Ctrl` `+` / `Ctrl` `-` / `Ctrl` `0` | Interface scale, as in the app |

The status line above the stage reports the scenario, preset, appearance, and
scale in effect, with the resolved appearance in parentheses when it is
following the system.

## Run the Presenter lab

`presenter-lab` opens the Presenter probe over a 500-Cue deck for review on
real hardware. Right and Left move Preview, Space takes Preview live, `F`
freezes Program, and `B` blacks it out. The footer shows live keydown-to-paint
p50, p95, and p99 for each command and surface:

```sh
cargo run --release -p ordo --example presenter-lab
```

`cargo xtask measure-presenter` runs the same lab for its native measurement.
Its measuring modes each run as a fresh process and write a JSON document:

- `--measure <out.json>`: the real operator runtime, the probe beside it, and
  the seeded 2,000-command stream, checked against a pure engine oracle.
- `--display-path <out.json> [passes]`: the CPU raster and retained display
  paths for one canonical frame.
- `--baseline <empty|preparation|presenter> <out.json>`: process memory of the
  real operator runtime (`empty_shell`); the runtime plus a synthetic
  10,000-entry library (`preparation_stand_in`, no Preparation UI or storage
  exists yet); and the runtime plus the 500-Cue probe with 500 canonical frames
  held through the bounded caches (`presenter_probe_in_runtime`).

The lab writes its temporary preferences file to the system temp directory and
removes it on exit.

## Verify

Install `cargo-deny`, then run:

```sh
cargo xtask verify
```

Ordo must build and operate without runtime internet access.
