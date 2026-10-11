#!/usr/bin/env python3
"""Prefill the M0 Windows 10 sign-off sheet from automation evidence.

Reads a `cargo xtask qualify` bundle `index.json` plus a church-PC
`results-*.zip` from `ordo-perf-kit.zip`'s `run.cmd`, applies the same gates
as `xtask/src/measure_presenter.rs` (p95 strictly below 50 ms, 512 MiB
qualification budget on baselines, 1 GiB absolute ceiling, zero hot-path
violations, oracle match), and writes a `WINDOWS-10-SIGNOFF.md` copy with the
Bundle identification and Machine sections filled.

Anything that needs eyes on site (display topology, scales, PowerPoint
version, checklist ticks, signatures) is left as an explicit placeholder.
The script never ticks a checklist row.

Usage:
    python3 prefill-signoff.py --index <index.json>
        --results <results-PC-STAMP.zip> [--ordo-exe <target/release/ordo.exe>]
        [--attempt N] [--out <sheet.md>]

Stdlib only.
"""

import argparse
import datetime
import hashlib
import json
import re
import sys
import zipfile
from pathlib import Path

MIB = 1024 * 1024
GIB = 1024 * MIB
P95_LIMIT_MS = 50.0
QUALIFICATION_BYTES = 512 * MIB
CEILING_BYTES = GIB
LATENCY_COMMANDS = ["navigation", "freeze", "blackout", "space"]
LATENCY_SURFACES = ["preview", "program"]
BASELINE_MODES = ["empty", "preparation", "presenter"]
NEEDS = "_(fill on site)_"


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def load_json_file(zf, name):
    try:
        return json.loads(zf.read(name).decode("utf-8"))
    except (KeyError, ValueError, UnicodeDecodeError):
        return None


def read_text(zf, name):
    try:
        return zf.read(name).decode("utf-8", errors="replace")
    except KeyError:
        return None


def zip_prefix(names):
    """Inner folder of run.cmd zips, or '' when files sit at the root."""
    tops = {n.split("/")[0] for n in names if "/" in n and not n.endswith("/")}
    if len(tops) == 1:
        return tops.pop() + "/"
    return ""


def pick_memory(sample, keys):
    for key in keys:
        value = (sample or {}).get(key)
        if isinstance(value, int) and not isinstance(value, bool):
            return value, key
    return None, None


def check_measure(doc):
    """Mirror measure_failures() in xtask/src/measure_presenter.rs."""
    failures = []
    if not isinstance(doc, dict):
        return ["keydown-to-paint run: no measure.json in results zip"]
    if isinstance(doc.get("error"), str):
        return ["keydown-to-paint run: " + doc["error"]]
    if doc.get("oracle_match") is not True:
        failures.append("keydown-to-paint run: final state diverged from the oracle")
    report = doc.get("measure")
    if not isinstance(report, dict):
        return failures + ["keydown-to-paint run: no latency table was recorded"]
    pairs = report.get("latency")
    if not isinstance(pairs, list):
        failures.append("keydown-to-paint run: no latency table was recorded")
    else:
        for command in LATENCY_COMMANDS:
            for surface in LATENCY_SURFACES:
                label = "keydown-to-paint %s %s" % (command, surface)
                pair = next(
                    (
                        p
                        for p in pairs
                        if isinstance(p, dict)
                        and p.get("command") == command
                        and p.get("surface") == surface
                    ),
                    None,
                )
                if pair is None:
                    failures.append(label + ": missing from the latency table")
                    continue
                if not isinstance(pair.get("samples"), int) or pair["samples"] <= 0:
                    failures.append(label + ": no samples were recorded")
                    continue
                p95 = pair.get("p95_ms")
                if not isinstance(p95, (int, float)) or isinstance(p95, bool):
                    failures.append(label + ": no p95 was recorded")
                elif p95 >= P95_LIMIT_MS:
                    failures.append(
                        label + ": p95 %.3f ms is at or over the 50 ms limit" % p95
                    )
    mem, source = pick_memory(
        report.get("memory"),
        ["peak_private_bytes", "peak_rss_bytes", "private_bytes", "rss_bytes"],
    )
    if mem is None:
        failures.append("keydown-to-paint run: process memory was not sampled")
    elif mem > CEILING_BYTES:
        failures.append(
            "keydown-to-paint run: %d MiB (%s) is over the 1 GiB absolute ceiling"
            % (mem // MIB, source)
        )
    hot = report.get("hot_path_violations")
    if hot != 0:
        if isinstance(hot, int):
            failures.append(
                "keydown-to-paint run: %d hot-path violation(s)" % hot
            )
        else:
            failures.append(
                "keydown-to-paint run: hot-path violations not recorded"
            )
    return failures


def check_baselines(files):
    """Mirror memory_failures() per baseline file. Returns (failures, summary)."""
    failures = []
    summary = []
    runs = {}
    for name, doc in sorted(files.items()):
        m = re.match(r"baseline-(\w+)-(\d+)\.json$", name)
        if not m:
            continue
        mode, run = m.group(1), int(m.group(2))
        if not isinstance(doc, dict):
            failures.append("baseline %s run %d: unreadable document" % (mode, run))
            continue
        if isinstance(doc.get("error"), str):
            failures.append("baseline %s run %d: %s" % (mode, run, doc["error"]))
            continue
        mem, source = pick_memory(
            doc, ["peak_private_bytes", "private_bytes", "peak_rss_bytes", "rss_bytes"]
        )
        if mem is None:
            failures.append(
                "baseline %s run %d: process memory was not sampled" % (mode, run)
            )
            continue
        runs.setdefault(run, {})[mode] = "%d MiB (%s)" % (mem // MIB, source)
        if mem > QUALIFICATION_BYTES:
            failures.append(
                "baseline %s run %d: %d MiB (%s) is over the 512 MiB qualification budget"
                % (mode, run, mem // MIB, source)
            )
        if mem > CEILING_BYTES:
            failures.append(
                "baseline %s run %d: %d MiB (%s) is over the 1 GiB absolute ceiling"
                % (mode, run, mem // MIB, source)
            )
    for run in sorted(runs):
        row = runs[run]
        summary.append(
            "run %d: %s"
            % (
                run,
                ", ".join("%s %s" % (m, row.get(m, "missing")) for m in BASELINE_MODES),
            )
        )
    if not summary:
        failures.append("baselines: no runs were recorded")
    return failures, summary


def parse_systeminfo(text):
    out = {}
    if not text:
        return out
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        key, value = key.strip().lower(), value.strip()
        if key == "os name":
            out["os_name"] = value
        elif key == "os version":
            out["os_version"] = value
        elif key.startswith("processor(s)") and value and not value.lower().endswith(
            ("installed.", "installed")
        ):
            out.setdefault("cpu", value)
        elif key.startswith("[01]") and "cpu" not in out and value:
            out["cpu"] = value
        elif key == "total physical memory":
            out["ram"] = value
        elif key == "system type":
            out["arch"] = value
    return out


def parse_dxdiag(text):
    """Processor, installed memory, DPI, and the first display's mode."""
    out = {}
    if not text:
        return out
    for line in text.splitlines():
        key, sep, value = line.partition(":")
        if not sep:
            continue
        key, value = key.strip().lower(), value.strip()
        if key == "processor":
            out.setdefault("cpu", " ".join(value.split()))
        elif key == "memory":
            out.setdefault("ram", value)
        elif key == "user dpi setting":
            out.setdefault("dpi", value)
        elif key == "current mode":
            out.setdefault("modes", []).append(value)
        elif key == "display topology":
            out.setdefault("topology", value)
    return out


def parse_kit_sha(text):
    """`KIT-SHA256.txt` lines (`<sha> *<name>`) as {name: sha}."""
    out = {}
    for line in (text or "").splitlines():
        sha, sep, name = line.strip().partition(" ")
        if sep:
            out[name.lstrip("*")] = sha.lower()
    return out


def parse_gpu_list(text):
    gpus = []
    if not text:
        return gpus
    name, driver = None, None
    for line in text.splitlines():
        stripped = line.strip()
        low = stripped.lower()
        if low.startswith("name"):
            if name:
                gpus.append((name, driver or "unknown driver version"))
            name = stripped.partition(":")[2].strip()
            driver = None
        elif low.startswith("driverversion"):
            driver = stripped.partition(":")[2].strip() or "unknown driver version"
    if name:
        gpus.append((name, driver or "unknown driver version"))
    return gpus


def parse_run_log(text):
    exits = {}
    started = finished = drive_line = None
    if not text:
        return exits, started, finished, drive_line
    for line in text.splitlines():
        m = re.match(r"(\S+): exit (\d+)\s*$", line.strip())
        if m:
            exits[m.group(1)] = int(m.group(2))
        elif line.startswith("started "):
            started = line[len("started "):].strip()
        elif line.startswith("finished "):
            finished = line[len("finished "):].strip()
        elif line.startswith("kit="):
            drive_line = line.strip()
    return exits, started, finished, drive_line


def main():
    ap = argparse.ArgumentParser(description="Prefill the M0 Windows sign-off sheet.")
    ap.add_argument("--index", required=True, help="bundle index.json")
    ap.add_argument("--results", required=True, help="results-*.zip from the USB stick")
    ap.add_argument("--ordo-exe", default=None, help="target/release/ordo.exe to hash")
    ap.add_argument("--attempt", type=int, default=1, help="attempt number for the row")
    ap.add_argument("--out", required=True, help="output sheet path")
    args = ap.parse_args()

    warnings = []
    index_path = Path(args.index)
    results_path = Path(args.results)
    try:
        index = json.loads(index_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        print("cannot read index.json: %s" % e, file=sys.stderr)
        return 2
    try:
        zf = zipfile.ZipFile(results_path)
        names = zf.namelist()
    except (OSError, zipfile.BadZipFile) as e:
        print("cannot read results zip: %s" % e, file=sys.stderr)
        return 2

    with zf:
        prefix = zip_prefix(names)
        get = lambda n: read_text(zf, prefix + n)
        get_json = lambda n: load_json_file(zf, prefix + n)

        kit_commit = (get("COMMIT.txt") or "").strip()
        measure = get_json("measure.json")
        display = get_json("display-path.json")
        baselines = {
            n[len(prefix):]: load_json_file(zf, n)
            for n in names
            if n.startswith(prefix + "baseline-") and n.endswith(".json")
        }
        sysinfo = parse_systeminfo(get("systeminfo.txt"))
        dxdiag = parse_dxdiag(get("dxdiag.txt"))
        kit_sha = parse_kit_sha(get("KIT-SHA256.txt"))
        gpus = parse_gpu_list(get("gpu.txt"))
        exits, started, finished, drive_line = parse_run_log(get("run-log.txt"))

    provenance = index.get("provenance", {})
    commit = provenance.get("commit", "?")
    dirty = provenance.get("dirty", "?")
    fixtures = provenance.get("fixtures", {})
    aggregate = fixtures.get("aggregate_sha256", "?") if isinstance(fixtures, dict) else "?"
    complete = index.get("complete", False)
    run = index.get("run", "?")
    started_unix = index.get("started_unix", 0)
    try:
        run_date = datetime.datetime.fromtimestamp(
            int(started_unix), datetime.timezone.utc
        ).strftime("%Y-%m-%d")
    except (ValueError, TypeError, OverflowError, OSError):
        run_date = NEEDS

    commit_mismatch = bool(kit_commit) and not commit.startswith(kit_commit)
    if commit_mismatch:
        warnings.append(
            "COMMIT.txt (%s) does not match provenance.commit (%s)"
            % (kit_commit, commit)
        )

    native_failures = check_measure(measure)
    base_failures, base_summary = check_baselines(baselines)
    native_failures.extend(base_failures)

    display_note = NEEDS
    if isinstance(display, dict):
        if isinstance(display.get("error"), str):
            native_failures.append("display path: " + display["error"])
            display_note = "error: " + display["error"]
        else:
            inner = display.get("display_path", display)
            if isinstance(inner, dict) and inner.get("passes"):
                display_note = "passes %s (see display-path.json)" % inner.get("passes")
            else:
                display_note = "see display-path.json"

    failing_steps = [
        "%s (%s)" % (s.get("name"), s.get("status"))
        for s in index.get("steps", [])
        if s.get("status") in ("fail", "missing")
    ]
    prov_problems = index.get("provenance_problems", [])

    pptx_name, pptx_sha = NEEDS, NEEDS
    for step in index.get("steps", []):
        for artifact in step.get("artifacts", []):
            if artifact.get("path") == "artifacts/emergency-fixture.pptx":
                pptx_name = "emergency-fixture.pptx"
                pptx_sha = artifact.get("sha256", "?")

    if args.ordo_exe and Path(args.ordo_exe).is_file():
        ordo_sha = sha256_file(args.ordo_exe)
    else:
        ordo_sha = NEEDS
        if args.ordo_exe:
            warnings.append("ordo exe not found: %s" % args.ordo_exe)

    kit_mismatch = False
    if not kit_sha:
        warnings.append("KIT-SHA256.txt absent from results; kit binaries not tied to the bundle")
    else:
        for name, expected in (("ordo.exe", ordo_sha), ("emergency-fixture.pptx", pptx_sha)):
            actual = kit_sha.get(name)
            if actual is None:
                warnings.append("%s is not listed in KIT-SHA256.txt" % name)
            elif expected != NEEDS and actual != expected.lower():
                kit_mismatch = True
                warnings.append(
                    "kit %s (%s) is not the bundle's (%s)" % (name, actual, expected)
                )

    os_ver = " ".join(
        x for x in (sysinfo.get("os_name"), sysinfo.get("os_version")) if x
    ) or str(provenance.get("os", {}).get("ver", NEEDS))
    gpu_str = (
        "; ".join("%s (driver %s)" % (n, d) for n, d in gpus)
        if gpus
        else str(provenance.get("gpu", NEEDS))
    )
    if not gpus:
        warnings.append("gpu.txt unparsable or absent; used provenance.gpu")
    if not sysinfo:
        warnings.append("systeminfo.txt absent; OS/CPU/RAM fall back to placeholders")

    perf_verdict = (
        "PASS (prefilled): all native gates hold"
        if not native_failures
        else "FAIL (prefilled):\n  - " + "\n  - ".join(native_failures)
    )
    automated_ok = complete and not native_failures and not failing_steps and not prov_problems
    attempt_result = "%s; automated rows: %s" % (
        NEEDS,
        "pass" if automated_ok else "FAIL",
    )
    gates_affected = (
        "none (prefilled): every native gate held"
        if not native_failures
        else "; ".join(native_failures)
    )
    fail_notes = "; ".join(failing_steps + prov_problems + native_failures) or "-"

    sheet = """# Milestone 0 Windows 10 22H2 packaged sign-off (prefilled)

> Generated by `docs/qualification/milestone-0/prefill-signoff.py` from the
> bundle index and the USB results zip. Rows marked (prefilled) come from
> automation; everything marked %(needs)s needs eyes on site. The script
> ticks no checklist row.
>
> - Index: `%(index)s` (SHA-256 `%(index_sha)s`)
> - Results: `%(results)s` (SHA-256 `%(results_sha)s`)
> - Run-log drive line: %(drive)s
> - Run-log window: started %(started)s / finished %(finished)s
> - Lab exits: %(exits)s
> - Baseline summary: %(bases)s
> - Display path: %(display)s
> - Startup: %(startup)s; oracle: %(oracle)s
> - Warnings: %(warnings)s

## Bundle identification

- Bundle commit (`provenance.commit` in `index.json`): %(commit)s
- Kit COMMIT.txt: %(kit)s
- Bundle `dirty` flag (must be `false`): %(dirty)s
- Bundle directory (`target/qualification/milestone-0/runs/<UTC>-<commit>/`): %(run)s
- Aggregate fixture SHA-256 (`provenance.fixtures`, aggregate): %(aggregate)s
- `index.json` SHA-256: %(index_sha)s
- `complete` value in `index.json` (must be `true`): %(complete)s
- Test date and local time: %(needs)s (PC clock in run-log: %(started)s; an offline PC clock may be wrong)
- Operator name or role: %(needs)s
- Observer name or role: %(needs)s
- Location of this completed sheet beside the bundle: %(needs)s

## Machine

- Windows edition, version, and build (must be 10 22H2, build 19045.x): %(os_ver)s (prefilled from systeminfo.txt; confirm build on site)
- `PROCESSOR_ARCHITECTURE`: %(arch)s (prefilled from systeminfo.txt)
- CPU model: %(cpu)s (prefilled from dxdiag.txt)
- Installed RAM: %(ram)s (prefilled from dxdiag.txt)
- GPU name and driver version (from `provenance.gpu`): %(gpu)s (prefilled from gpu.txt)
- Free storage before test: %(needs)s
- Windows display scale: %(dpi)s (prefilled from dxdiag.txt; confirm on site)
- Ordo Interface scale: %(needs)s
- Operator display resolution and refresh rate: %(needs)s (dxdiag modes: %(modes)s)
- Congregation display or projector resolution and refresh rate: %(needs)s
- Display topology: %(topology)s (prefilled from dxdiag.txt; confirm on site)
- Installed PowerPoint version (`File > Account > About PowerPoint`): %(needs)s
- Network confirmed unavailable: [ ]
- Release `ordo` binary SHA-256 (`target/release/ordo.exe`): %(ordo)s

## Checklist

Mark each row with the attempt number it passed on. Record findings in the
attempts table, not here.

- [ ] Release `ordo` binary launches from the built artifact, offline.
- [ ] Operator window and congregation window run the walking skeleton with the same fixture-based canonical frame.
- [ ] Task supervision: closing the operator window closes the congregation window, and worker failures appear as structured results.
- [ ] Storage: the SQLite worker opens the clean fixture state and stays responsive during navigation.
- [ ] Canonical rendering: Program matches the canonical golden composition at the operator's display scale.
- [ ] Navigation: Left/Right, Up/Down, Freeze, Blackout, and Space take effect on Program.
- [ ] Display loss: unplugging or disabling the congregation display fails safely to black and reports only to the operator; reconnect follows the black-first recovery path.
- [ ] Performance: static Program idle CPU, Presenter open time, and memory stay within the adopted gates, and the observations are recorded.
  - Prefilled verdict: %(perf)s
- [ ] Emergency PowerPoint fixture opens and presents in the installed PowerPoint version (`MILESTONES.md` T0.9 `[MANUAL]`). Record the generated file name and SHA-256 below.

Emergency PowerPoint fixture:

- Generated file name: %(pptx_name)s (prefilled from bundle artifacts)
- Generated file SHA-256: %(pptx_sha)s (prefilled from bundle artifacts)
- Opens in the installed PowerPoint version without a repair prompt: %(needs)s
- Sections and slide order correct: %(needs)s
- Full-frame images, notes, and safety slides correct: %(needs)s
- Slide Show runs on the congregation display without missing or altered content: %(needs)s

## Attempts

| Attempt # | Date | Bundle commit | Result (pass / fail / ambiguous) | Failure notes | Rerun of # |
|---|---|---|---|---|---|
| %(attempt)d | %(needs)s | %(commit)s | %(result)s | %(failnotes)s | |

## Hardware hard-gate outcome

- [ ] Release blocked. No release is produced until the gate is met.
- [ ] Supported hardware profile revised explicitly, with the revision and its
      rationale recorded in `MVP-PLAN.md` or the decision ledger.

- Gate(s) affected: %(gates)s
- Profiling evidence reference: `%(results_name)s` + bundle `%(run)s`
- Outcome selected: %(needs)s
- Rationale: %(needs)s

## Findings

- Finding: %(needs)s
- Blocking: [ ] Yes  [ ] No
- Corrective build and bundle commit: %(needs)s
- Rerun attempt #: %(needs)s

## Sign-off

- Checklist complete (every row passed on the attempt recorded): [ ] Yes  [ ] No
- No unresolved blocking or ambiguous findings: [ ]
- Hardware hard-gate outcome recorded (section above): [ ]
- Operator sign-off (name, date): %(needs)s
- Observer sign-off (name, date): %(needs)s
""" % dict(
        needs=NEEDS,
        index=args.index,
        index_sha=sha256_file(index_path),
        results=args.results,
        results_sha=sha256_file(results_path),
        drive=drive_line or NEEDS,
        started=started or NEEDS,
        finished=finished or NEEDS,
        exits=(
            ", ".join("%s=%d" % kv for kv in sorted(exits.items())) or NEEDS
        ),
        bases="; ".join(base_summary) or NEEDS,
        display=display_note,
        startup=(
            str((measure or {}).get("startup_ms", NEEDS))
            if isinstance(measure, dict)
            else NEEDS
        ),
        oracle=(
            str((measure or {}).get("oracle_match", NEEDS))
            if isinstance(measure, dict)
            else NEEDS
        ),
        warnings="; ".join(warnings) or "none",
        commit=commit,
        kit=kit_commit or NEEDS,
        dirty=dirty if not isinstance(dirty, bool) else str(dirty).lower(),
        run=run,
        aggregate=aggregate,
        complete=str(bool(complete)).lower(),
        run_date=run_date,
        os_ver=os_ver,
        arch=sysinfo.get("arch", NEEDS),
        cpu=dxdiag.get("cpu") or sysinfo.get("cpu", NEEDS),
        ram=dxdiag.get("ram") or sysinfo.get("ram", NEEDS),
        dpi=dxdiag.get("dpi", NEEDS),
        modes="; ".join(dxdiag.get("modes", [])) or "none recorded",
        topology=dxdiag.get("topology", NEEDS),
        gates=gates_affected,
        gpu=gpu_str,
        ordo=ordo_sha,
        perf=perf_verdict,
        pptx_name=pptx_name,
        pptx_sha=pptx_sha,
        attempt=args.attempt,
        result=attempt_result,
        failnotes=fail_notes.replace("|", "\\|"),
        results_name=results_path.name,
    )

    out_path = Path(args.out)
    out_path.write_text(sheet, encoding="utf-8")
    print("wrote %s" % out_path)
    print("perf verdict: %s" % ("PASS" if not native_failures else "FAIL"))
    for warning in warnings:
        print("warning: %s" % warning)
    if commit_mismatch or kit_mismatch:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
