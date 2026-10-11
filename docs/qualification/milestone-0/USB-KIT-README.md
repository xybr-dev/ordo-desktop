# Ordo perf kit — church-PC instructions (M0)

You need: this stick, the church PC with its usual operator +
congregation displays connected, and the installed PowerPoint noted.
No internet, no Rust, nothing to install. About 5-8 minutes hands-off
(budget 10).

## On the church PC

1. Plug in the stick. Open it, confirm `COMMIT.txt` matches the commit
   you were told to qualify. If it does not, stop and ask.
2. Note before running: free storage, Windows display scale, Ordo
   Interface scale, both display resolutions/refresh rates, topology,
   PowerPoint version (`File > Account > About PowerPoint`).
3. Close other apps. Double-click `run.cmd`.
4. Keep hands off the keyboard and mouse until it says `DONE`.
   Windows open and close by themselves; touching them slows or
   invalidates the timing.
5. When `DONE` appears, check the last lines: every run should say
   `exit 0`. If any says otherwise, leave the window open and note it.
6. Safe-eject the stick. Done on site — nothing is installed, the lab
   deletes its temp preferences on exit.

Bring home: the new `results-<PC>-<date>.zip` on the stick.

## Back home

1. Copy the `results-*.zip` off the stick.
2. Prefill the sign-off sheet from your qualify bundle:
   ```
   python3 docs/qualification/milestone-0/prefill-signoff.py \
     --index target/qualification/milestone-0/runs/<UTC>-<commit>/index.json \
     --results results-<PC>-<date>.zip \
     --ordo-exe target/release/ordo.exe \
     --attempt 1 \
     --out sheet-attempt1.md
   ```
   It fills Bundle identification, Machine, the perf verdict, the
   PowerPoint artifact SHA, and the attempt row. It ticks nothing.
3. Fill every `_(fill on site)_` from your notes, tick checklist rows
   only for what you actually observed, complete Attempts / Findings /
   hard-gate outcome / signatures.
4. Store the completed sheet beside that exact bundle run, with the
   results zip and checksums. Do not commit completed sheets.

## If something goes wrong

- `presenter-lab.exe is missing`: the stick copy is incomplete; recopy.
- `Cannot write ... check read-only switch`: the stick's lock switch is
  on, or it is full.
- A run reports `exit 1` or a window stays open: note which run
  (`measure`, `display-path`, `baseline-<mode>-<run>`), keep
  `run-log.txt` and that run's `.stdout/.stderr.txt`, report it as a
  failed attempt — do not delete or rerun over it.
- `dxdiag` takes a minute: normal, wait.
