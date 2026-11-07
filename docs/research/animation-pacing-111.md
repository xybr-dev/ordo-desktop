# Research: animation frame pacing in release (#111)

Status: measurement and diagnosis for issue #111, steps 1 and 2 (measure in
release, find the cost). Nothing here changes a curve, a duration, a decision
record or a milestone. Measured on 2026-11-07 on the owner's machine: release
build, AMD Radeon integrated graphics, primary monitor at 75 Hz (13.3 ms per
frame).

## How it was measured

`live-control-lab --measure-animations <out.json> [--size WxH] [--scale N]
[--cues 30|500] [--list|--grid]` opens the real Presenter, lets the frame store
settle, then presses each animation in turn with idle gaps and writes one JSON
document. `--cues 500` uses the generated 500-Cue snapshot of
`--measure-memory`; `--cues 30` uses the 33-Cue `normal-mass` fixture. `--list`
selects the List Cue view (the default is Grid).

- **Interval.** The time between the paint stamps of two consecutive drawn
  frames inside one scripted press (`FrameTimer` counts the frames over 1.5x
  16.7 ms). GPUI on Windows presents with a sync interval of 0 and draws on a
  vsync thread, so this is the draw cadence, not the GPU present time.
- **Cost per frame.** `render` is `PresenterView::render` (a render probe
  around it). `layout` runs from the end of that render to the first prepaint
  canvas: it includes the layout request of every element, nested views and
  text measurement. `prepaint` and `paint` are the spans between the two
  stamping canvases that bracket the window.
- **Store.** Frames composed during the run, and the frames whose render saw a
  new composed frame (a thumbnail arrival).
- **Animations.** Timeline follow glide (Next, NextItem, Last, PreviousItem,
  First across group boundaries); rail overlay open and close (rail unpinned);
  Cue-group fold and unfold; output fades (EnableOutput, Take, Blackout). A
  diagnostic fifth run repeats the output fades with every group folded.

Matrix: 1366x768 and 1920x1080 x Grid and List x 100% and 200% x 30 and 500
Cues, 16 fresh release processes, each about 45 s. Raw documents were kept
outside the repository.

## Limits of the measurement

- **The window size was not honoured in the "before" runs.** On this machine
  the lab window opened maximized whatever `--size` said (`window_bounds` was
  `Maximized`, viewport 1920x1025), so the 1366x768 and 1920x1080 rows of the
  "before" figures are the same window and act as a repeat run. The 200% scale
  did change the layout (an effective 960x512). The cause was a Windows setting
  on the measuring machine that opens every app maximized, not Ordo. Later runs
  were made with it off: 1366x768 gives a 1366x768 viewport and 1920x1080 gives
  1920x1048 (the display clamps it). The lab warns when its window opens
  maximized, and the report records `window_maximized`.
- The window was on screen and active in every run (`window_active_at_start`
  true, no inactive frames counted).
- The monitor is 75 Hz, so one frame is 13.3 ms. The 16.7 ms bar of the issue
  is more lenient than what the display shows; a frame that costs more than
  13.3 ms lands on the next vsync.
- Debug against release was not measured: only release was run. The release
  figures already explain the symptom.
- GPU present time is not measured. Thumbnail upload cost shows only through
  the CPU frame cost.

## Results

Per cell: p95 of the frame interval, the worst interval, and the intervals over
33 ms, across the eight 30-Cue and eight 500-Cue cells (size x view x scale).

| Animation | Cues | interval p95 (ms) | interval max (ms) | over 33 ms | frame cost p95 (ms) |
| --- | --- | --- | --- | --- | --- |
| timeline glide | 30 | 14.5 to 15.4 | 17.6 to 25.5 | 0 | 7.2 to 9.4 |
| timeline glide | 500 | 68.6 to 83.8 | 76 to 95 | 64 to 84 of 84 to 104 frames | 52.6 to 71.8 |
| rail overlay | 30 | 14.2 to 23.7 | 14.6 to 25.2 | 0 | 8.1 to 9.5 |
| rail overlay | 500 | 67.4 to 89.9 | 67.5 to 89.9 | 8 to 12 of 16 to 20 frames | 51.4 to 68.1 |
| group fold | 30 | 14.5 to 23.2 | 15.8 to 29.2 | 0 | 7.1 to 11.2 |
| group fold | 500 | 69.3 to 78.4 | 69.8 to 81.2 | 30 to 33 of 38 to 41 frames | 52.5 to 71.8 |
| output fades | 30 | 14.6 to 16.8 | 15.7 to 27.8 | 0 | 7.3 to 10.3 |
| output fades | 500 | 68.2 to 81.0 | 69.4 to 94.7 | 40 to 52 of 51 to 63 frames | 52.5 to 69.2 |
| output fades, all groups folded | 500 | 15.6 to 30.9 | 24.4 to 31.5 | 0 | 10.8 to 19.2 |

The idle median interval at 30 Cues is 13.3 to 13.5 ms (75 Hz), with 0 to 5% of
intervals over 16.7 ms outside a few spikes; at 500 Cues the median is 66 to 67
ms, five vsyncs, about 15 fps, and every interval is over 16.7 ms. At 500 Cues
the per-frame phases are about render 5 to 6.5, layout 34 to 49, prepaint 5 to
7.5 and paint 7 to 10 ms (p95). At 30 Cues they are 1.3 to 1.6, 4 to 6, 0.5 to
0.8 and 1.3 to 3 ms.

Grid against List and 100% against 200% matter much less than the Cue count.
200% is the worse one at 500 Cues (layout up to 49 ms), and the rail overlay
and the first frames of a fold are the worst single frames at 30 Cues (the
23 to 29 ms outliers).

## Findings against the hypotheses

1. **Debug build.** Probably the cause of the jaggedness at the size of a real
   Mass, but a debug run was not measured, so this is inferred. At 30 Cues
   release holds the display rate: no interval over 33 ms in any of the 40
   30-Cue animation runs, p95 within about 1 ms of the refresh interval except
   for a few rail-overlay and fold spikes. At 500 Cues release is not smooth
   either.
2. **Thumbnail work mid-animation.** Not the cause. At 500 Cues a glide
   composed 120 to 140 frames and 23 to 34 frames saw an arrival, yet the
   intervals of those frames (max 69 to 85 ms) are no worse than the rest of the
   same run (p50 66.5 ms). The output fades (no scrolling) are as slow as the
   glide. Thumbnail jobs add a frame or two of CPU, but they are not what holds
   the frame rate down.
3. **Per-frame rebuild cost scaling with Cue count.** Confirmed, and it is the
   cause. Every animated frame re-renders the whole Presenter, and the
   timeline lays out all of it: layout went from about 5 ms (33 Cues, 15 groups)
   to 34 to 49 ms (500 Cues, 60 groups), and total frame cost from about 9 to
   about 55 to 70 ms, against a 13.3 ms budget. The render of the element tree
   (5 to 6 ms) is only a tenth of it. Folding every group (hiding every Cue
   tile) brings the same fades from 55 to 12 to 19 ms, and to under 13 ms at
   200% (layout 8 ms, interval 13.4 ms), so the cost is dominated by the Cue
   tiles that are laid out whether or not they are on screen. Output fades
   (which never move the timeline) are as slow as the glide, which also shows
   the cost is per frame of any animation, not about the glide itself.
4. **Where the offset is applied.** No sign of doubled or dropped frames from
   the prepaint offset: the glide's intervals are as regular as the output
   fades (p50 13.3 ms at 30 Cues, and the 500-Cue glide is uniformly 66 ms
   like every other animation at that size). Only confirmed as far as the
   interval pattern goes; the offset's one-frame lag was not isolated.

## Recommended fixes

In order of benefit, none applied here:

1. **Render only the groups on screen (virtualise the timeline).** Lay out and
   render the visible groups plus a margin, and stand in a spacer of the known
   height for the rest. Expected: layout and prepaint fall to roughly the
   30-Cue figures (about 5 and 1 ms), so a 500-Cue frame lands near 9 ms, inside
   13.3 ms, with no animation change. This addresses the 500-Cue case in every
   animation, including the output fades and the rail overlay.
2. **Cache unchanged subtrees.** The header, dock and rail do not change during
   a glide or a fade. Marking them cached (so GPUI reuses their layout) and
   keeping a folded group as one small element removes the per-frame text
   measurement of Cue captions. Expected: a further drop even at 30 Cues
   (about 9 ms to about 5 ms), and the folded case already shows the shape of
   the gain (55 to 12 to 19 ms).
3. **Keep the rail overlay and the fades from re-rendering the timeline.** An
   animation that only changes the Program or Preview opacity, or the rail slide,
   should not re-render the timeline view at all. This overlaps with 2 and is
   cheaper to land first if a separate entity for the timeline is acceptable.
4. **Judge with the display rate.** Set the bar against the display's refresh
   (13.3 ms here), not a fixed 16.7 ms, and record the measured refresh in
   `report.json` (step 4 of the issue).

Not worth doing first: thumbnail scheduling (the data does not show it as the
cost), and any change to curves or durations (the frames are late, not the curve
wrong).

## Reproducing

```text
cargo build --release -p ordo --example live-control-lab
cargo run --release -p ordo --example live-control-lab -- \
  --measure-animations out.json --size 1366x768 --scale 100 --cues 500 [--list]
```

Keep the window on screen and unoccluded while it runs (about 45 s).

## After timeline virtualisation (step 3)

The timeline now lays out only the groups within one timeline height (its own
bounds, not the window's) of the visible span, plus the Preview and Program
groups; every other open group is one spacer `div` of exactly its height. A
group that is folding or unfolding counts at the height it has on screen at
that moment, so the groups below it stay laid out while it moves. The height is computed by the layout model, per
authored length rounded to device pixels as GPUI rounds them, so a spacer and
the group it replaces are the same height to the device pixel and nothing moves
when one swaps for the other. Every group is still one child at its own index.
Accessibility: a group away from the viewport is a spacer, so its header (the
Expand or Collapse button) is not in the accessibility tree until the group
comes near the viewport. Screen-reader users reach any group through the
timeline's active-descendant name; whether that is enough is an open decision
for the owner.
Measured on 2026-11-07 with the same release lab and monitor (75 Hz, 13.3 ms),
the same 16-process matrix, now with real window sizes (1366x768 and 1920x1048
viewport). Raw documents were kept outside the repository.

Per animation: p95 of the frame interval and p95 of the frame cost (CPU), as the
range across the eight cells (size x view x scale), and the intervals over 33 ms
in all eight cells together. "Before" is the table above (500 Cues, maximized
window in every cell).

| Animation (500 Cues) | interval p95 before (ms) | interval p95 after (ms) | frame cost p95 before (ms) | frame cost p95 after (ms) | over 33 ms before | over 33 ms after |
| --- | --- | --- | --- | --- | --- | --- |
| timeline glide | 68.6 to 83.8 | 14.6 to 19.9 | 52.6 to 71.8 | 6.3 to 17.1 | 579 of 579 | 0 of 2145 |
| rail overlay | 67.5 to 89.9 | 14.0 to 17.0 | 51.4 to 69.2 | 7.6 to 12.3 | 88 of 88 | 0 of 386 |
| group fold | 69.3 to 78.4 | 14.5 to 17.0 | 52.5 to 71.8 | 4.8 to 12.1 | 255 of 255 | 0 of 830 |
| output fades | 68.2 to 81.0 | 14.3 to 17.0 | 52.5 to 69.2 | 4.0 to 12.6 | 352 of 352 | 0 of 1639 |
| output fades, all folded | 15.6 to 30.9 | 14.9 to 20.5 | 10.8 to 19.2 | 10.2 to 18.4 | 0 | 0 |

At 30 Cues nothing regressed and the cost fell a little (frame cost p95 4 to 10
ms against 7 to 11 ms, layout 2.3 to 5.8 against 4.1 to 6.3 ms); no interval is
over 33 ms either way.

Layout at 500 Cues went from 34 to 49 ms (p95) to 2 to 10 ms, close to the 30-Cue
figure, as the diagnosis predicted. The remaining cost is the rest of the frame
(render 5 to 6 ms of the whole Presenter, prepaint, paint) and it grows with
the window and the scale: the worst cells are 100% at 1920x1048 (glide p95 19.9
ms in List and 16.7 ms in Grid, frame cost up to 17 ms), and 200% is the
cheapest because its effective window is smaller.

Against the 13.3 ms budget the 500-Cue p95 is near it but not under it: most
cells sit at 14 to 17 ms (one to two vsyncs late at the tail), the worst at 19.9
ms.

### Regression bar in report.json (step 4, implemented)

`cargo xtask measure-presenter` now runs the animation measurement once (release
`live-control-lab --measure-animations`, 1366x768, 100%, 500 Cues, Grid) and
stores the document under `native.animations` in `report.json`. Frame pacing is
judged against the refresh interval measured in that same run, not a fixed 16.7
ms: before the scripted animations the lab draws frames back to back for about
a second with no state change, and the median interval is `refresh_ms`
(`refresh_hz`, `refresh_samples`; `null` with a `refresh_note` when fewer than 20
intervals arrive). It is the Presenter's own idle draw cadence, with no OS
display query, so it equals the display refresh only when an idle frame fits in
one vsync and the native vsync source is in use. The probe's own frame cost p95
is recorded as `refresh_cpu_p95_ms`. Each animation also records
`p95_over_refresh` and `cpu_p95_over_refresh`, and `limits_ms` carries `hitch`
(33), `refresh` and `p95_bar` (1.5 x refresh, echoed; xtask owns the factor and
fails if the echo disagrees by more than 0.05 ms).

Gates: the refresh must be plausible (4 to 17.5 ms) and the probe's frame cost
p95 at most 0.9 of it, because a slow idle frame (one that costs more than a
vsync) would otherwise stretch the cadence and raise its own bar. All five animations (`timeline_glide`, `rail_overlay`, `group_fold`,
`output_fades`, `output_fades_all_folded`) must be present with a boolean
`applicable`; all but `rail_overlay` must apply, so at least four are judged.
Per animation that applied: no interval over 33 ms and interval p95 at most 1.5
refresh intervals. An unmeasured run (missing or errored document, no refresh,
no intervals) fails.

On a CI runner (`GITHUB_ACTIONS=true`) the animation lab is not run: a GitHub
Windows runner has no real display, the vsync source falls back to a sleep
timer (the first run read a 52 ms "refresh" with 116 intervals over 33 ms), and
WARP frames are slow, so nothing it measured could be judged. The document
records `native.animations` as `{"skipped": ...}` and `animations_verdict` as
`{"judged": false, ...}`; locally it is `{"judged": true, ...}` and findings
fail the run. The p95 frame cost against one refresh interval
is printed in the run summary and not judged: it fails today in the 100% glide
cells, and its fix (caching the unchanged header, dock and rail, and keeping the
rail and fades from re-rendering the timeline, recommendations 2 and 3) is still
open in #111.

First run with the gate (2026-11-07, same machine): refresh 13.26 ms (75.4 Hz,
74 samples), p95 bar 19.89 ms; interval p95 of the five animations 14.7 to 16.0
ms (1.11 to 1.20 refresh), no interval over 33 ms, frame cost p95 0.69 to 1.15
refresh intervals.

The earlier proposal read: no interval over 33 ms, interval p95 at most 1.5
refresh intervals, frame cost p95 at most one refresh interval (reported only).

## After step 3b (overscan, Cue model, pinned rail)

Three things were tried against the frame cost that was left (render 1.4 to 2.4
ms, layout 7 to 10 ms, prepaint 1 to 1.6 ms, paint 2.7 to 3.6 ms at 100% and
1920x1048). Measured on 2026-11-07 on the same monitor (75 Hz, 13.3 ms) with the
release lab, window left alone, one run per cell and build, the two builds
alternating cell by cell. "Before" is the build at the head of the previous
step, run in this session, because the "after virtualisation" figures above are
ranges over cells, not per cell.

**Overscan.** The groups laid out around the visible span are now a named
constant, `OVERSCAN`, a fraction of the timeline height above and below. At
1920x1080, 100%, 500 Cues, frame cost p95 of the timeline glide was 14.8 ms
(List) and 13.8 ms (Grid) at 1.0, 13.4 and 12.5 at 0.5, 12.4 and 12.2 at 0.25.
Differences of 1 ms are inside the run-to-run spread, but the direction held in
both views. A quarter is kept: layout is built from the offset the frame is
drawn at (a wheel or scrollbar move notifies and the next render lays out around
the new offset, however far it moved, and the follow glide sets its offset in
prepaint, after layout), so the margin only absorbs rounding. A test scrolls
three timeline heights by wheel in both directions and presses End and Home,
frame by frame, in four sizes and both views, and checks that no group in view
is a spacer.

**Cue model.** `timeline_model` (all 500 `CueModel`s) took 0.47 ms at the median
and 0.73 ms p95 (temporary stamps, 862 frames, List 1920x1080 100%), a third of
the render phase and 5% of the frame. It is not cached. The model is consumed by
the timeline, the frame store has no change counter to key a cache on and
runtime availability is read from the engine for each Cue, so a cache that can
never be stale needs a new version counter in the store and a clone of 500
models per frame, and would save about 0.3 ms. Not worth the staleness risk.

**The pinned rail.** Taking the rail out of the frame (an experiment) cut layout
by about 3 ms and frame cost by 3 to 4 ms in the 1920x1080 100% cells: its 60
rows were laid out on every frame to draw the same thing. Taking the timeline
out instead left layout at 4 ms (header, rail and dock), so the rest was the
timeline. The rail only changes when Preview moves to another Mass item or the
window changes size. It is now its own `Entity` (`rail::RailView`), drawn with
`Entity::cached` while pinned; the Presenter syncs the current model into it
each frame and it redraws only when that differs. Focus handle, F6 region and
key context stay on the Presenter. Layout of the rail, when it does redraw, is
counted under prepaint, which is why prepaint p95 rose in the glide and fold
cells (0.9 to 3.1 ms): it is the frames where Preview moved. The header and the
dock were left alone (the dock draws the fades, the header is cheap).

Frame cost p95 (ms), before to after, 500 Cues unless noted; the interval p95
stayed at 14.3 to 18.1 ms in every cell and no interval went over 33 ms:

| Cell | glide | rail overlay | group fold | output fades | all folded |
| --- | --- | --- | --- | --- | --- |
| 1920x1080 100% List | 15.4 to 11.2 | 13.1 to 10.9 | 12.5 to 9.9 | 12.6 to 7.8 | 16.1 to 11.5 |
| 1920x1080 100% Grid | 13.9 to 10.7 | 12.6 to 9.3 | 11.3 to 10.1 | 9.9 to 6.1 | 15.2 to 10.5 |
| 1920x1080 200% List | 5.9 to 5.9 | 9.7 to 9.6 | 5.0 to 4.8 | 4.4 to 4.0 | 10.4 to 11.7 |
| 1920x1080 200% Grid | 6.0 to 5.9 | 8.1 to 9.9 | 5.0 to 5.2 | 4.4 to 4.2 | 10.4 to 10.4 |
| 1366x768 100% Grid | 10.3 to 8.4 | 11.3 to 9.2 | 9.2 to 7.9 | 8.7 to 5.8 | 14.7 to 13.8 |
| 1366x768 100% Grid, 30 Cues | 6.2 to 4.9 | 6.4 to 5.1 | 5.3 to 4.8 | 5.6 to 4.2 | 5.3 to 4.0 |

Layout p95 in the 1920x1080 100% List cell: glide 9.5 to 4.5 ms, fades 7.0 to
3.4 ms. The 200% cells are unchanged because the rail is not pinned there (the
effective window is too narrow), and those cells already sat under the
refresh interval. After the change the 100% cells at 1920x1080 are at 6 to 11.5
ms against the 13.3 ms refresh, the glide in List being the worst at 11.2 ms;
every 1920x1080 100% cell that was over one refresh interval at p95 is
now under it. "All folded" at 1366x768 stays at 13.8 ms, just over.

One run, 1366x768 100% Grid all folded, came back with 13 intervals over 33 ms
(interval p95 41.9 ms) although its frame cost was normal. Three reruns of the
same build and cell gave frame cost p95 10.7 to 11.1 ms, interval p95 15.8 to
16.0 ms and no interval over 33 ms; the first run is taken as the window being
disturbed, and is kept here as measured. The idle probe frame
(`refresh_cpu_p95_ms`, List 1920x1080 100%) fell from 10.6 to 6.9 ms.

Curves and durations were not touched.
