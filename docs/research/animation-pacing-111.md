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

- **The window size was not honoured (fixed in step 3).** On this machine the
  lab window opened maximized whatever `--size` said (`window_bounds` was
  `Maximized`, viewport 1920x1025), so the 1366x768 and 1920x1080 rows of the
  "before" figures are the same window and act as a repeat run. The 200% scale
  did change the layout (an effective 960x512). Step 3 made a measuring window
  non-maximizable, which keeps its bounds: 1366x768 gives a 1366x768 viewport
  and 1920x1080 gives 1920x1048 (the display clamps it).
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

The timeline now lays out only the groups within one viewport of the visible
span, plus the Preview and Program groups; every other open group is one spacer
`div` of exactly its height. The height is computed by the layout model, per
authored length rounded to device pixels as GPUI rounds them, so a spacer and
the group it replaces are the same height to the device pixel and nothing moves
when one swaps for the other. Every group is still one child at its own index.
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

### Proposed regression bar for report.json (not implemented)

Record the measured refresh interval in the report and judge frame pacing
against it, not against a fixed 16.7 ms. For the 500-Cue animation measurement,
proposed gates, per animation and cell: no interval over 33 ms (0 today); p95
interval at most 1.5 refresh intervals (20 ms at 75 Hz; the worst cell is
19.9 ms, so it passes with almost no margin); p95 frame cost at most one
refresh interval (13.3 ms; fails today only in the 100% glide cells, 14.4 to
17.1 ms). Gate on the first two and report the third, until the remaining frame
cost (recommendations 2 and 3 above, caching the unchanged header,
dock and rail, and keeping the rail and fades from re-rendering the timeline) is
tackled.
