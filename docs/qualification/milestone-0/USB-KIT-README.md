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
| `ordo.exe` | Release Ordo, for the launch and performance checks by hand. |
| `walking-skeleton-lab.exe` | The operator and congregation walking skeleton, for checks 2-6. A development lab, not Presenter Mode. |
| `emergency-fixture.pptx` | Emergency PowerPoint fixture (T0.9). |
| `vcruntime140*.dll` | Runtime libraries for the executables. |
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

Run the programs straight from the stick. Tick each row on the sign-off
sheet only for what you actually saw.

Checks 2-6 use `walking-skeleton-lab.exe`. It is a development lab, not
Presenter Mode: one lab window (title, Preview, Program, Cue position,
status line, key help) plus the congregation output on the projector. It
loads a small Mass-shaped deck of 7 Cues in 4 items: Entrance Song (3 Cues),
Penitential Act (1), First Reading (2), Blackout (1). Keys work with the lab
window focused: Right/Left move Preview by one Cue, Up/Down jump to the
previous/next item, Space takes Preview live, `F` freezes, `B` blacks out,
`F6` enables output, `F8` re-reads the displays, `F9` disables output, `N`
writes a note to storage.

1. **Launch offline:** `ordo.exe` opens with the network unplugged.
2. **Both windows:** start `walking-skeleton-lab.exe`. Focus the lab window
   and press `F6` to enable output (the projector is black first). Press
   Space to take the current Cue live. The projector shows the same
   canonical slide as the lab's Program monitor.
3. **Navigation:** Right/Left move Preview, Up/Down jump between items,
   Space takes live, `F` freezes Program (navigation continues, the
   projector holds), `B` blacks out. Each takes effect on Program as
   described. **Storage:** the lab's status line shows the temporary
   storage as ready; press `N` while navigating. A note is added and
   navigation never stalls.
4. **Canonical rendering:** the projected slides' text placement and
   wrapping look right at the projector's scale: song title, two- and
   three-line lyrics, Cebuano response, reading identification and
   passage.
5. **Display loss:** with content live, unplug or disable the projector.
   Program goes black and the lab's status line (operator only) shows the
   output is disconnected. Nothing appears on another display. If the
   status does not change within a second or two, press `F8`. Reconnect,
   then press `F8` and `F6`: output returns black, and content returns
   only after Space.
6. **Supervision:** close Ordo's operator window (the shell window, not the
   lab window). The projector window and the lab close, and the process
   exits with no error dialog.
7. **Performance by eye:** with `ordo.exe` running and a static window,
   Task Manager CPU for Ordo stays low (under 5 %); note memory in Task Manager.
8. **Emergency PowerPoint:** open `emergency-fixture.pptx`. No repair
   prompt. Sections and slide order are right, slides fill the frame,
   notes and the black safety slides are present. Run Slide Show on the
   projector to the end. The deck is a synthetic test pattern: five
   striped colour images (the stripes should reach all four edges in Slide
   Show) and four black operator-only slides (1, 5, 8, 9) with notes.
   Sections are Safety, Identification, Entrance Song, Penitential Act
   response, First Reading, Safety.

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
   PowerPoint SHA-256, the lab's SHA-256, and the attempt row, and checks that
   the kit's `ordo.exe`, `walking-skeleton-lab.exe`, and deck are the
   bundle's. It ticks nothing. It exits 1 if
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
