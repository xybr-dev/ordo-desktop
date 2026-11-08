# Phase 1 manual gate

Status: open. Every Phase 1 `[AUTO]`, `[UI-AUTO]`, and `[NATIVE-AUTO]` row is
evidenced in `MILESTONES.md`; the rows below are the `[MANUAL]` checks still
open, plus two rechecks the Phase 1 gate audit (8 Nov 2026) added. The owner
runs them and ticks the matching `MILESTONES.md` boxes. Record each result in
section 3 with the date, the build commit, and the PC.

## 1. Build under test

Run every script on a packaged release build from a clean `main` checkout:

```powershell
git status --short          # must be empty
cargo build --release -p ordo
target\release\ordo.exe
```

Record `git rev-parse --short HEAD` in section 3. Scripts A, B, and E work on
the dev PC; script C needs the church projector; scripts D and F want a second
display (the church projector, or any external monitor on the dev PC).

The fixture is the sanitized Normal Mass that `ordo.exe` opens in the
Presenter. It has no private content of its own, so script E's privacy check
looks for operator-only elements on the congregation display, not for real
names.

## 2. Scripts

### A. T1.4 — Packaged UI matches the approved Presenter references

`MILESTONES.md` T1.4: "The packaged UI matches the approved Presenter
references in hierarchy, state clarity, focus visibility, and dim-room
usability." The references are listed in `DESIGN.md` "Approved References":
`docs/mockups/presenter-three-column-v2.html`, `timeline-modes-v2.html`,
`presenter-high-scale-v2.html`, and `semantic-palette-v2.html`.

1. Open the four reference pages beside `ordo.exe`. Press **Present**.
2. Hierarchy: compare the session header, the Output Dock (Program, Preview,
   output status, Freeze, Blackout, primary action), the timeline, and the
   rail with `presenter-three-column-v2.html`. The Dock stays visible while
   the timeline scrolls.
3. Timeline modes: switch Grid, List, and 16:9 and compare with
   `timeline-modes-v2.html`. Groups show Mass-item titles only and scroll the
   full timeline to the group rather than filtering it.
4. State clarity: take a Cue live (Space), Freeze (F), Blackout (B), and
   Disable. Each state reads at a glance from the Dock and the header, in
   words as well as colour.
5. Focus visibility: press Tab and F6 through every region. The focus ring is
   visible on every focused element, including Cue tiles.
6. High scale: set the Interface scale to its maximum and resize the window
   to 1366x768. Compare with `presenter-high-scale-v2.html`; canonical frames
   keep their composition.
7. Dim room: switch Windows to dark mode and dim the screen to its lowest
   comfortable level, or run in a dark room. Text, the live/frozen/blackout
   states, and the focus ring stay readable without glare.

Pass when every step matches the references or a difference is recorded and
accepted in section 3.

### B. T1.5 — Keyboard-only and Narrator announcements

`MILESTONES.md` T1.5: "Keyboard-only and Windows screen-reader checks announce
expected transitions politely and unexpected safety events urgently without
duplicate queues." Use Narrator (Ctrl+Win+Enter turns it on and off).

1. Unplug or ignore the mouse. Start Narrator, then `ordo.exe`.
2. Reach **Present** with the keyboard only and open the Presenter.
3. Press F6 to cycle regions and Tab within each. Narrator reads each region,
   and each Cue tile with its position, text or type, and behaviour
   (project, wait, hold, Blackout).
4. Expected transitions, polite: press Space, F, B, and the auto-play key A.
   Narrator reads one short status per change, after what it was reading.
   Pressing the same key again with no change reads nothing new.
5. Refusals, polite: press A on a liturgical Cue (auto-play is refused
   there). Narrator reads the refusal once.
6. Safety events, urgent: with output enabled on a second display, unplug it.
   Narrator interrupts to read the display-loss message once. Replug it and
   listen for the reconnect status.
7. No duplicate queues: during steps 4 to 6, no message is read twice and no
   stale message is read after a newer one.
8. Open the shortcut reference (Ctrl+/) and close it with Esc. Focus returns
   to where it was, and Narrator reads it.

Pass when every step holds. If Narrator reads nothing at all for a step, record
it as a finding (it may be a GPUI AccessKit gap, not an Ordo one).

### C. T1.6 — Church projector cabling (issues #129, #133)

`MILESTONES.md` T1.6: "On the church projector's real cabling (including any
extender or switch), unplugging and replugging it is recognized as the same
display, and a different display is refused."

At the church, the projector is fed through an HDMI splitter that also feeds a
choir mirror display. Windows keeps the second display while any splitter
output is connected, so a loss is seen only when every splitter output is
disconnected.

1. Present, Enable Output, take a Cue live.
2. Disconnect every splitter output (or the splitter's input). Program goes
   black at once, with no key pressed, and nothing appears on the operator
   display.
3. Reconnect. The header says the projector is back. Output stays black until
   Space; Space resumes on the projector.
4. Different display (#133): repeat steps 1 and 2, then connect a different
   monitor in its place. Output stays disconnected and the header says the
   display was not recognized. Space does not open output there.
5. Record the display names the header shows in section 3.

### D. T0.5 recheck — pointer hiding over the congregation window

Decision 313 (#131) added pointer hiding after T0.5's manual gate. T0.5's
owner check (9 Oct 2026) tested the locator strip with the display left and
right only.

1. Present, Enable Output.
2. Move the pointer onto the congregation display. The pointer is hidden there
   and the locator strip shows on the operator display.
3. Move it back. The pointer reappears on the operator display at once,
   including on a fast move.
4. In Windows display settings, place the second display **above**, then
   **below** the main one, and repeat steps 2 and 3 for each.

### E. Phase 1 gate — Normal Mass by keyboard alone

`MILESTONES.md` Phase 1 gate: "A human can run the sanitized Normal Mass
fixture from keyboard alone without exposing private content or losing state
clarity."

1. Mouse unplugged. Second display connected.
2. Present, Enable Output, and run the whole Normal Mass from the first Cue to
   the last with the keyboard only: Space, the arrow keys, F, B, A where the
   fixture auto-plays, and Esc for any overlay.
3. Privacy: at no point does the congregation display show an operator-only
   element (Preview, the timeline, the rail, the recovery strip, a focus
   ring, a toast, or the pointer).
4. State clarity: at every step you can tell from the operator display what
   is on Program, what is next, and whether output is live, frozen, or black.
5. Exit Presenter at the end. Ordo returns to Preparation.

### F. T1.7 recheck — real kill and relaunch

The T1.7 E2E restarts inside the test host. This script kills the real process.
`cargo xtask recovery-drill` scripts the same check against the real packaged
`ordo.exe` (Phase 1 gate, Recovery Drill). It needs two displays, because
Enable Output is refused with one, so it is a local or church-kit check and
not a CI job. It builds `ordo` in release, stages it with the Normal Mass
fixture, runs every launch with `APPDATA` pointing at a temporary directory
(never real Parish data), and for Live, Frozen and Blackout presses Present,
Enable Output, Space (and F or B), waits for the journal on disk to show the
state, hard-kills the process, relaunches, and checks that the app reopened in
the recovered Presenter (a new session whose first point carries the
interrupted state, no rejected journal) and that Space takes the recovered
Preview live. It writes `target/ordo-recovery-drill/report.json` and exits
non-zero on any failure. It does not look at the projector: steps 3 and 4
below (black output, strip text) stay a human check. With one display,
`cargo xtask recovery-drill --single-display` runs only an output-off case.
Run the command, and then the steps by hand, at least once on the church PC.

1. Present, Enable Output, take a Cue live.
2. In a terminal: `taskkill /F /IM ordo.exe`.
3. Start `ordo.exe` again. It opens straight into the Presenter with no
   dialog, Program black, the projector black, and the strip "Presentation
   recovered · Congregation output is black".
4. Press Space. Preview goes live on the projector and the strip goes away.
5. Repeat steps 1 to 4 after F (Freeze) and after B (Blackout).

## 3. Results

| Script | Date | Commit | PC | Result | Notes and findings |
|---|---|---|---|---|---|
| A | | | | | |
| B | | | | | |
| C | | | | | |
| D | | | | | |
| E | | | | | |
| F | | | | | |
