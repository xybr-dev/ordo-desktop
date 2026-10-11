# Ordo perf kit — church-PC instructions (M0)

You need: this stick, the church PC with its usual operator and
congregation displays connected, and a pen for the sign-off sheet.
No internet, no Rust, nothing to install. About 5 minutes hands-off for
the automated part, then about 20 minutes of checks by hand.

## What is on the stick

| File | Purpose |
|---|---|
| `run.cmd` | Automated performance run. Double-click it. |
| `presenter-lab.exe` | Performance harness that `run.cmd` drives. |
| `ordo.exe` | Release Ordo, for the checks by hand. |
| `emergency-fixture.pptx` | Emergency PowerPoint fixture (T0.9). |
| `vcruntime140*.dll` | Runtime libraries for both executables. |
| `COMMIT.txt`, `BUNDLE.txt` | Commit and qualification bundle this kit was built from. |
| `KIT-SHA256.txt` | Checksums. `run.cmd` refuses to run if any file differs. |
| `previous/` | Earlier results. Do not delete. |

## On the church PC

### 1. Before running

1. Plug in the stick. Confirm `COMMIT.txt` matches the commit you were
   told to qualify. If it does not, stop and ask.
2. Write down the **real** date and time. The church PC's clock is often
   wrong because it is kept offline; the results folder name uses the
   PC clock.
3. Note: free storage, Windows display scale, both display
   resolutions and refresh rates, topology, PowerPoint version
   (`File > Account > About PowerPoint`), and whether the network is
   unplugged and Wi-Fi off.

### 2. Automated run

1. Close other apps. Double-click `run.cmd`.
2. `[0/4] checking kit files` must pass. If it says `MISSING` or
   `CHANGED`, the stick copy is bad: stop, do not run, recopy at home.
3. Keep hands off the keyboard and mouse until it says `DONE`. Windows
   open and close by themselves; touching them invalidates the timing.
4. When `DONE` appears, every run must say `exit 0`. If any does not,
   leave the window open and note which one.

### 3. Checks by hand

Run `ordo.exe` straight from the stick. Tick each row on the sign-off
sheet only for what you actually saw.

1. **Launch offline:** `ordo.exe` opens with the network unplugged.
2. **Both windows:** enable congregation output; Program on the projector
   shows the same canonical frame as the operator's Program monitor.
3. **Navigation:** Left/Right, Up/Down, `F` (Freeze), `B` (Blackout), and
   Space each take effect on Program.
4. **Canonical rendering:** the projected slide matches the golden
   composition (text placement and wrapping look right at this scale).
5. **Display loss:** unplug or disable the projector. Program fails to
   black and only the operator sees a message. Reconnect: output comes
   back black first, then content only when you resume.
6. **Supervision:** close the operator window. The congregation window
   closes too, and Ordo exits without an error dialog.
7. **Performance by eye:** with a static slide, Task Manager CPU for Ordo
   stays low (under 5 %); note memory in Task Manager.
8. **Emergency PowerPoint:** open `emergency-fixture.pptx`. No repair
   prompt. Sections and slide order are right, slides fill the frame,
   notes and the black safety slides are present. Run Slide Show on the
   projector to the end.

### 4. Finish

Safe-eject the stick. Nothing is installed; the lab deletes its temporary
preferences on exit.

Bring home: the new `results-<PC>-<date>.zip` and your notes.

## Back home

1. Copy the `results-*.zip` off the stick.
2. Prefill the sign-off sheet from the bundle named in `BUNDLE.txt`:
   ```
   python -I docs/qualification/milestone-0/prefill-signoff.py \
     --index target/qualification/milestone-0/runs/<BUNDLE.txt>/index.json \
     --results results-<PC>-<date>.zip \
     --ordo-exe <cargo target dir>/release/ordo.exe \
     --attempt 1 \
     --out sheet-attempt1.md
   ```
   It fills Bundle identification, Machine, the perf verdict, the
   PowerPoint SHA-256, and the attempt row, and checks that the kit's
   `ordo.exe` and deck are the bundle's. It ticks nothing. It exits 1 if
   the kit does not match the bundle.
3. Fill every `_(fill on site)_` from your notes (use the real time, not
   the PC clock), tick checklist rows only for what you observed, and
   complete Attempts, Findings, hard-gate outcome, and signatures.
4. Store the completed sheet beside that exact bundle run, with the
   results zip and checksums. Do not commit completed sheets.

## If something goes wrong

- `presenter-lab.exe is missing`: the stick copy is incomplete; recopy.
- `Kit files do not match KIT-SHA256.txt`: a file is damaged or was
  changed; recopy the whole kit.
- `Cannot write ... check read-only switch`: the stick's lock switch is
  on, or it is full.
- A run reports `exit 1` or a window stays open: note which run
  (`measure`, `display-path`, `baseline-<mode>-<run>`), keep
  `run-log.txt` and that run's `.stdout/.stderr.txt`, and report it as a
  failed attempt. Do not delete it or rerun over it.
- `dxdiag` takes a minute: normal, wait.
- A check by hand fails: record it as a finding on the sheet with what
  you saw. Do not retry until it is written down.
