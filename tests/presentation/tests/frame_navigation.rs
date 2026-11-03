//! 500-Cue navigation under slow, superseded and failed frame jobs (T1.3 WP4).
//!
//! The Engine decides what Program and Preview want; a deterministic harness
//! plays the cache owner and the task executor: it pins the Program and
//! Preview frames, requests them and a preload window from the
//! `FrameScheduler`, completes jobs out of order (some many steps late, ~5%
//! failing) and reports failures back with `Engine::mark_frame_unavailable`.
//! Two models stand in for the monitors: each only changes when the frame it
//! wants is complete in the cache, and otherwise keeps its last complete frame.
//!
//! This proves the bounded-count half of the memory envelope (cache entries,
//! jobs in flight, composed frames held) on forward, reverse and random
//! navigation. The native private-memory envelope (Presenter probe, about
//! 198.5 MiB private for 500 Cues, `docs/plans/T0.6-implementation-plan.md`)
//! is measured by `cargo xtask measure-presenter`, not here. Run with
//! `--nocapture` to print the maxima.
//!
//! Composition is the real `rendering::compose`, run once per distinct frame
//! (120 of them) and cloned for each later job: composing 20,000 times in a
//! debug build would dominate the run without testing the scheduler.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use domain::presentation::{
    Cue, CueBehavior, FrameId, MassItem, MassItemKind, MassSection, PresentationSnapshot,
    SnapshotId, ThemeId, ThemeKind, ThemeRef,
};
use presentation::commands::Command;
use presentation::engine::{Availability, Engine, UnavailableSource};
use presentation::state::{LiveState, Millis, ProgramContent};
use rendering::{
    Completion, FRAME_CAPACITY, Frame, FrameCache, FrameEntry, FrameScheduler, FrameSlot,
    PRELOAD_RADIUS, TemplateContent, TemplateKind, Ticket, UnavailableReason, compose,
};

const CUES: usize = 500;
const SECTIONS: usize = 6;
const ITEMS: usize = 60;
const DISTINCT_FRAMES: usize = 120;
/// Distinct frames that may fail in one run (~8% of 120).
const FAILURE_CAP: usize = 10;

struct XorShift(u64);

impl XorShift {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }
}

fn uuid(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn frame_id(n: usize) -> FrameId {
    uuid(0x50_0000 + n as u64).parse().unwrap()
}

/// Invented placeholder text, never real liturgical content.
fn content(n: usize) -> TemplateContent {
    const KINDS: [TemplateKind; 9] = [
        TemplateKind::Lyrics,
        TemplateKind::Response,
        TemplateKind::ReadingIdentification,
        TemplateKind::ReadingPassage,
        TemplateKind::Psalm,
        TemplateKind::SongIdentification,
        TemplateKind::CelebrationTitle,
        TemplateKind::GeneralText,
        TemplateKind::Black,
    ];
    let kind = KINDS[n % KINDS.len()];
    let body = if kind == TemplateKind::Black {
        String::new()
    } else {
        format!("Placeholder line {n} alpha\nPlaceholder line {n} beta")
    };
    TemplateContent {
        kind,
        header: Some(format!("Placeholder header {n}")),
        body,
        footer: Some(format!("Placeholder footer {n}")),
        image: None,
    }
}

struct Fixture {
    snapshot: Arc<PresentationSnapshot>,
    /// Frame of Cue `i`, when it is a Project Cue.
    frame_of: Vec<Option<FrameId>>,
    /// Cues projecting each frame.
    cues_of: BTreeMap<FrameId, Vec<usize>>,
    /// The result of the real `compose` per frame.
    composed: HashMap<FrameId, Frame>,
    unavailable_cues: usize,
}

fn fixture() -> Fixture {
    let theme: ThemeId = uuid(0x400).parse().unwrap();
    let kinds = [
        MassItemKind::Song,
        MassItemKind::Reading,
        MassItemKind::Psalm,
        MassItemKind::ResponseSequence,
        MassItemKind::GeneralPresentation,
        MassItemKind::Rite,
    ];
    let mut flat = 0usize;
    let mut frame_of = Vec::new();
    let mut unavailable_cues = 0;
    let mut sections = Vec::new();
    for section in 0..SECTIONS {
        let mut items = Vec::new();
        for item in section * ITEMS / SECTIONS..(section + 1) * ITEMS / SECTIONS {
            // 20 items of 9 Cues and 40 of 8: exactly 500.
            let size = if item < 20 { 9 } else { 8 };
            let mut cues = Vec::new();
            for _ in 0..size {
                let (behavior, context, frame) = match flat {
                    i if i % 50 == 7 => {
                        unavailable_cues += 1;
                        (
                            CueBehavior::Unavailable {
                                reason: "placeholder asset missing".to_owned(),
                            },
                            None,
                            None,
                        )
                    }
                    i if i % 23 == 5 => (
                        CueBehavior::OperatorTrigger,
                        Some("placeholder trigger".to_owned()),
                        None,
                    ),
                    i if i % 31 == 11 => (CueBehavior::Hold, None, None),
                    i if i % 37 == 13 => {
                        (CueBehavior::Wait, Some("placeholder wait".to_owned()), None)
                    }
                    i => {
                        let frame = frame_id(i * 7 % DISTINCT_FRAMES);
                        (
                            CueBehavior::Project {
                                frame,
                                duration_seconds: None,
                            },
                            None,
                            Some(frame),
                        )
                    }
                };
                frame_of.push(frame);
                cues.push(
                    Cue::new(
                        uuid(0x1_0000 + flat as u64).parse().unwrap(),
                        format!("Cue {flat}"),
                        context,
                        behavior,
                    )
                    .unwrap(),
                );
                flat += 1;
            }
            items.push(
                MassItem::new(
                    uuid(0x1000 + item as u64).parse().unwrap(),
                    format!("Item {item}"),
                    kinds[item % kinds.len()],
                    None,
                    None,
                    cues,
                )
                .unwrap(),
            );
        }
        sections.push(
            MassSection::new(
                uuid(0x100 + section as u64).parse().unwrap(),
                format!("Section {section}"),
                items,
            )
            .unwrap(),
        );
    }
    assert_eq!(flat, CUES);
    let snapshot = PresentationSnapshot::new(
        uuid(1).parse::<SnapshotId>().unwrap(),
        "Synthetic 500-Cue Mass".to_owned(),
        theme,
        vec![ThemeRef {
            id: theme,
            name: "Fallback".to_owned(),
            kind: ThemeKind::Fallback,
        }],
        sections,
    )
    .unwrap();
    assert_eq!(snapshot.cue_count(), CUES);
    assert_eq!(snapshot.item_count(), ITEMS);

    let mut cues_of: BTreeMap<FrameId, Vec<usize>> = BTreeMap::new();
    for (cue, frame) in frame_of.iter().enumerate() {
        if let Some(frame) = frame {
            cues_of.entry(*frame).or_default().push(cue);
        }
    }
    let composed = cues_of
        .keys()
        .map(|frame| {
            let n = (0..DISTINCT_FRAMES)
                .find(|n| frame_id(*n) == *frame)
                .unwrap();
            let frame_value = compose(&content(n)).unwrap_or_else(|e| panic!("frame {n}: {e}"));
            (*frame, frame_value)
        })
        .collect();
    Fixture {
        snapshot: Arc::new(snapshot),
        frame_of,
        cues_of,
        composed,
        unavailable_cues,
    }
}

/// One job the executor holds: a ticket, when it finishes, and what it yields.
struct Job {
    ticket: Ticket<FrameId>,
    due: u64,
    /// A job the harness already abandoned (the scheduler forgot it); its late
    /// completion must be ignored.
    zombie: bool,
    fail: bool,
}

#[derive(Default)]
struct Maxima {
    cache_len: usize,
    ready_in_cache: usize,
    in_flight: usize,
    composed_held: usize,
    jobs_pending: usize,
}

struct Counters {
    accepted: usize,
    ignored_late: usize,
    failures_marked: usize,
    commands: usize,
    program_changes: usize,
    preview_changes: usize,
    preview_stale_held: usize,
}

struct Harness<'a> {
    fx: &'a Fixture,
    engine: Engine,
    cache: FrameCache<FrameId>,
    scheduler: FrameScheduler<FrameId>,
    rng: XorShift,
    step: u64,
    clock: u64,
    jobs: Vec<Job>,
    /// Last key requested per slot (what `Accepted` must match).
    requested: HashMap<FrameSlot, FrameId>,
    failed: BTreeSet<FrameId>,
    /// Per frame: whether its jobs fail (decided at its first job).
    planned_failures: HashMap<FrameId, bool>,
    /// What the monitors draw: the last complete frame they were given.
    program_shown: Option<(FrameId, Arc<Frame>)>,
    preview_shown: Option<(FrameId, Arc<Frame>)>,
    maxima: Maxima,
    counters: Counters,
}

impl<'a> Harness<'a> {
    fn new(fx: &'a Fixture, seed: u64) -> Self {
        Self {
            fx,
            engine: Engine::new(Arc::clone(&fx.snapshot)),
            cache: FrameCache::new(FRAME_CAPACITY),
            scheduler: FrameScheduler::new(),
            rng: XorShift(seed),
            step: 0,
            clock: 0,
            jobs: Vec::new(),
            requested: HashMap::new(),
            failed: BTreeSet::new(),
            planned_failures: HashMap::new(),
            program_shown: None,
            preview_shown: None,
            maxima: Maxima::default(),
            counters: Counters {
                accepted: 0,
                ignored_late: 0,
                failures_marked: 0,
                commands: 0,
                program_changes: 0,
                preview_changes: 0,
                preview_stale_held: 0,
            },
        }
    }

    fn program_key(&self) -> Option<FrameId> {
        match self.engine.state().program() {
            ProgramContent::Frame { frame, .. } => Some(frame),
            ProgramContent::Black => None,
        }
    }

    /// The frame the Preview monitor wants: the Preview Cue's frame unless the
    /// Cue is not presentable (then the monitor is black).
    fn preview_key(&self) -> Option<FrameId> {
        let preview = self.engine.state().preview();
        match self.engine.cue_availability(preview) {
            Availability::Presentable => self.fx.frame_of[preview],
            Availability::Unavailable(_) => None,
        }
    }

    /// Frames of the Cues within `PRELOAD_RADIUS` of Preview, nearest first.
    fn preload_keys(&self) -> Vec<FrameId> {
        let preview = self.engine.state().preview();
        let mut keys = Vec::new();
        for distance in 1..=PRELOAD_RADIUS {
            for cue in [preview + distance, preview.wrapping_sub(distance)] {
                if cue < CUES
                    && self.engine.cue_availability(cue) == Availability::Presentable
                    && let Some(frame) = self.fx.frame_of[cue]
                {
                    keys.push(frame);
                }
            }
        }
        keys
    }

    fn enqueue(&mut self, ticket: Ticket<FrameId>) {
        // Mostly prompt, sometimes many steps late.
        let delay = match self.rng.below(100) {
            0..=44 => 0,
            45..=84 => 1 + self.rng.below(3),
            85..=94 => 4 + self.rng.below(12),
            _ => 40 + self.rng.below(160),
        };
        // Composition is deterministic, so a frame that fails once fails for
        // every job: decide it once per frame (~5% of frames, capped).
        let key = ticket.key();
        let planned = self.planned_failures.values().filter(|fail| **fail).count();
        let fail = match self.planned_failures.get(&key) {
            Some(fail) => *fail,
            None => {
                let fail = planned < FAILURE_CAP && self.rng.below(100) < 5;
                self.planned_failures.insert(key, fail);
                fail
            }
        };
        self.jobs.push(Job {
            ticket,
            due: self.step + delay,
            zombie: false,
            fail,
        });
    }

    /// Pins, requests and preloads for the Engine's current state, then
    /// drops what no slot wants any more.
    fn sync(&mut self) {
        let program = self.program_key();
        let preview = self.preview_key();
        self.cache.pin(FrameSlot::Program, program);
        self.cache.pin(FrameSlot::Preview, preview);
        for (slot, key) in [(FrameSlot::Program, program), (FrameSlot::Preview, preview)] {
            if let Some(key) = key {
                self.requested.insert(slot, key);
                if let Some(ticket) = self.scheduler.request(slot, key, &self.cache) {
                    self.enqueue(ticket);
                }
            }
        }
        let keys = self.preload_keys();
        for ticket in self.scheduler.request_preload(keys, &self.cache) {
            self.enqueue(ticket);
        }
        self.abandon_obsolete();
    }

    /// Obsolete jobs are either completed right away (late: must be ignored)
    /// or abandoned; half of the abandoned ones still finish later as ghosts.
    fn abandon_obsolete(&mut self) {
        for ticket in self.scheduler.obsolete() {
            self.scheduler.abandon(&ticket);
            if let Some(index) = self.jobs.iter().position(|job| job.ticket == ticket) {
                if self.rng.below(2) == 0 {
                    self.jobs.swap_remove(index);
                } else {
                    self.jobs[index].zombie = true;
                }
            }
        }
    }

    /// Completes every job that is due, in random order.
    fn complete_due(&mut self) -> bool {
        let mut marked = false;
        let mut due = Vec::new();
        let mut index = 0;
        while index < self.jobs.len() {
            if self.jobs[index].due <= self.step {
                due.push(self.jobs.swap_remove(index));
            } else {
                index += 1;
            }
        }
        while !due.is_empty() {
            let job = due.swap_remove(self.rng.below(due.len() as u64) as usize);
            let key = job.ticket.key();
            let result = if job.fail {
                Err(UnavailableReason::DecodeFailed)
            } else {
                Ok(self.fx.composed[&key].clone())
            };
            let before = self.cache.peek(&key).is_some();
            let completion = self.scheduler.complete(job.ticket, result, &mut self.cache);
            match completion {
                Completion::Accepted => {
                    assert!(!job.zombie, "an abandoned job was accepted");
                    // Latest wins: an accepted Program or Preview job is the
                    // newest request of its slot.
                    if matches!(job.ticket.slot(), FrameSlot::Program | FrameSlot::Preview) {
                        assert_eq!(
                            self.requested.get(&job.ticket.slot()),
                            Some(&key),
                            "a superseded {:?} job was accepted",
                            job.ticket.slot()
                        );
                    }
                    self.counters.accepted += 1;
                    if job.fail {
                        self.failed.insert(key);
                        self.counters.failures_marked += 1;
                        self.engine.mark_frame_unavailable(
                            key,
                            UnavailableReason::DecodeFailed,
                            Millis(self.clock),
                        );
                        marked = true;
                    }
                }
                Completion::Ignored => {
                    // A dropped result leaves no trace in the cache.
                    assert_eq!(
                        self.cache.peek(&key).is_some(),
                        before,
                        "an ignored completion changed the cache"
                    );
                    self.counters.ignored_late += 1;
                }
            }
        }
        marked
    }

    /// The monitors change only when the frame they want is complete.
    fn update_monitors(&mut self) {
        let program_wanted = self.program_key();
        match program_wanted {
            None => self.program_shown = None,
            Some(key) => {
                if let Some(FrameEntry::Ready(frame)) = self.cache.peek(&key)
                    && self.program_shown.as_ref().map(|(shown, _)| *shown) != Some(key)
                {
                    self.program_shown = Some((key, Arc::clone(frame)));
                    self.counters.program_changes += 1;
                }
            }
        }
        let preview_wanted = self.preview_key();
        match preview_wanted {
            None => self.preview_shown = None,
            Some(key) => {
                if let Some(FrameEntry::Ready(frame)) = self.cache.peek(&key) {
                    if self.preview_shown.as_ref().map(|(shown, _)| *shown) != Some(key) {
                        self.preview_shown = Some((key, Arc::clone(frame)));
                        self.counters.preview_changes += 1;
                    }
                } else if self.preview_shown.is_some() {
                    self.counters.preview_stale_held += 1;
                }
            }
        }
    }

    fn assert_step(&mut self, context: &str) {
        // Bounded resources.
        let len = self.cache.len();
        assert!(
            len <= FRAME_CAPACITY + 2,
            "{context}: cache holds {len} entries"
        );
        let in_flight = self.scheduler.in_flight();
        assert!(
            in_flight <= 2 + 2 * PRELOAD_RADIUS,
            "{context}: {in_flight} jobs in flight"
        );

        // Program is black or a complete frame, and once composed it is the
        // Engine's Program frame.
        let program = self.program_key();
        match (&self.program_shown, program) {
            (None, None) => {}
            (None, Some(key)) => assert!(
                !matches!(self.cache.peek(&key), Some(FrameEntry::Ready(_))),
                "{context}: Program frame is ready but not shown"
            ),
            (Some(_), None) => panic!("{context}: Program shows a frame while Engine is black"),
            (Some((shown, frame)), Some(key)) => {
                if *shown == key {
                    assert_eq!(**frame, self.fx.composed[&key], "{context}: wrong Program");
                    // Pinned: the frame the congregation sees is never evicted.
                    assert!(
                        matches!(self.cache.peek(&key), Some(FrameEntry::Ready(held))
                            if Arc::ptr_eq(held, frame) || **held == **frame),
                        "{context}: Program frame left the cache"
                    );
                } else {
                    // Waiting for the new frame: the old one is complete.
                    assert_eq!(**frame, self.fx.composed[shown], "{context}");
                    assert!(
                        !matches!(self.cache.peek(&key), Some(FrameEntry::Ready(_))),
                        "{context}: Program frame is ready but not shown"
                    );
                }
            }
        }

        // Preview never regresses to a superseded frame: it shows the wanted
        // frame once complete, an older complete frame while waiting, or black.
        let preview = self.preview_key();
        match (&self.preview_shown, preview) {
            (None, None) => {}
            (None, Some(key)) => assert!(
                !matches!(self.cache.peek(&key), Some(FrameEntry::Ready(_))),
                "{context}: Preview frame is ready but not shown"
            ),
            (Some(_), None) => panic!("{context}: Preview shows a frame while Engine is black"),
            (Some((shown, frame)), Some(key)) => {
                assert_eq!(**frame, self.fx.composed[shown], "{context}");
                if *shown != key {
                    assert!(
                        !matches!(self.cache.peek(&key), Some(FrameEntry::Ready(_))),
                        "{context}: Preview frame is ready but not shown"
                    );
                }
            }
        }

        // Frames held: ready entries plus monitor frames the cache dropped.
        let ready = self
            .fx
            .cues_of
            .keys()
            .filter(|key| matches!(self.cache.peek(key), Some(FrameEntry::Ready(_))))
            .count();
        let mut held: Vec<&Arc<Frame>> = Vec::new();
        for shown in [&self.program_shown, &self.preview_shown]
            .into_iter()
            .flatten()
        {
            held.push(&shown.1);
        }
        let extra = held
            .iter()
            .filter(|frame| {
                !self.fx.cues_of.keys().any(|key| {
                    matches!(self.cache.peek(key), Some(FrameEntry::Ready(c)) if Arc::ptr_eq(c, frame))
                })
            })
            .count();
        self.maxima.cache_len = self.maxima.cache_len.max(len);
        self.maxima.ready_in_cache = self.maxima.ready_in_cache.max(ready);
        self.maxima.in_flight = self.maxima.in_flight.max(in_flight);
        self.maxima.composed_held = self.maxima.composed_held.max(ready + extra);
        self.maxima.jobs_pending = self.maxima.jobs_pending.max(self.jobs.len());

        // Every reported failure is Unavailable for each Cue that projects it.
        for key in &self.failed {
            for &cue in &self.fx.cues_of[key] {
                assert!(
                    matches!(
                        self.engine.cue_availability(cue),
                        Availability::Unavailable(UnavailableSource::Runtime(
                            UnavailableReason::DecodeFailed
                        ))
                    ),
                    "{context}: Cue {cue} of failed frame is {:?}",
                    self.engine.cue_availability(cue)
                );
            }
        }
    }

    /// One command, then the cache owner's and executor's reaction.
    fn command(&mut self, command: Command, label: &str) {
        self.step += 1;
        self.clock += 20 + self.rng.below(200);
        self.counters.commands += 1;
        let _ = self.engine.apply(command, Millis(self.clock));
        self.sync();
        let marked = self.complete_due();
        if marked {
            // A failure changes what Preview wants (and may move auto-play).
            self.sync();
        }
        self.update_monitors();
        let context = format!("{label} step {} {command:?}", self.step);
        self.assert_step(&context);
    }

    /// A step with no command: only the executor makes progress.
    fn idle(&mut self) {
        self.step += 1;
        self.sync();
        if self.complete_due() {
            self.sync();
        }
        self.update_monitors();
        let context = format!("idle step {}", self.step);
        self.assert_step(&context);
    }

    fn preview(&self) -> usize {
        self.engine.state().preview()
    }

    fn live(&self) -> LiveState {
        self.engine.state().live()
    }
}

/// Space, Freeze and Blackout interleaved with navigation (~12% of commands).
fn toggle(rng: &mut XorShift) -> Option<Command> {
    match rng.below(100) {
        0..=4 => Some(Command::Take),
        5..=7 => Some(Command::ToggleFreeze),
        8..=10 => Some(Command::ToggleBlackout),
        11 => Some(Command::EnableOutput),
        _ => None,
    }
}

#[test]
fn five_hundred_cue_navigation_stays_bounded_under_slow_and_failed_jobs() {
    let started = std::time::Instant::now();
    let fx = fixture();
    assert_eq!(fx.unavailable_cues, 10);
    assert_eq!(fx.cues_of.len(), DISTINCT_FRAMES);
    let mut h = Harness::new(&fx, 0xC0FF_EE15_BADD_CAFE);
    h.command(Command::EnableOutput, "setup");
    h.command(Command::Take, "setup");

    // Phase 1: forward sweeps with First at the end.
    for _ in 0..6_000 {
        let command = toggle(&mut h.rng).unwrap_or_else(|| match h.rng.below(100) {
            _ if h.preview() == CUES - 1 => Command::First,
            0..=84 => Command::Next,
            85..=94 => Command::NextItem,
            _ => Command::GoTo(h.preview() + 1 + h.rng.below(3) as usize),
        });
        h.command(command, "forward");
        if h.rng.below(10) == 0 {
            h.idle();
        }
    }
    assert!(h.preview() > 0 || h.counters.commands > CUES);

    // Phase 2: reverse sweeps with Last at the start.
    for _ in 0..6_000 {
        let command = toggle(&mut h.rng).unwrap_or_else(|| match h.rng.below(100) {
            _ if h.preview() == 0 => Command::Last,
            0..=84 => Command::Previous,
            85..=94 => Command::PreviousItem,
            _ => Command::GoTo(h.preview().saturating_sub(1 + h.rng.below(3) as usize)),
        });
        h.command(command, "reverse");
        if h.rng.below(10) == 0 {
            h.idle();
        }
    }

    // Phase 3: random jumps and steps.
    for _ in 0..8_000 {
        let command = toggle(&mut h.rng).unwrap_or_else(|| match h.rng.below(10) {
            0 => Command::GoTo(h.rng.below(CUES as u64 + 4) as usize),
            1..=3 => Command::Next,
            4..=6 => Command::Previous,
            7 => Command::NextItem,
            8 => Command::PreviousItem,
            _ => Command::GoTo(h.rng.below(CUES as u64) as usize),
        });
        h.command(command, "random");
    }

    // Drain: every outstanding job finishes (or is ignored) and nothing is
    // left in flight beyond the live requests.
    for _ in 0..250 {
        h.idle();
    }
    assert!(h.counters.commands >= 20_000, "{}", h.counters.commands);
    assert!(
        !h.failed.is_empty(),
        "no failure was injected; the run proves nothing about them"
    );
    // The Engine's overlay holds exactly the reported failures.
    for cue in 0..CUES {
        let failed = h.fx.frame_of[cue].is_some_and(|frame| h.failed.contains(&frame));
        let runtime = matches!(
            h.engine.cue_availability(cue),
            Availability::Unavailable(UnavailableSource::Runtime(_))
        );
        assert_eq!(failed, runtime, "Cue {cue}");
    }
    assert!(
        h.counters.ignored_late > 0,
        "no late completion was exercised"
    );
    assert!(h.counters.preview_stale_held > 0, "Preview never waited");
    assert!(h.maxima.ready_in_cache <= FRAME_CAPACITY + 2);
    let _ = h.live();

    println!(
        "frame navigation: {} commands, {} jobs accepted, {} ignored late, {} frames failed",
        h.counters.commands,
        h.counters.accepted,
        h.counters.ignored_late,
        h.failed.len()
    );
    println!(
        "  Program changes {}, Preview changes {}, Preview waits {}",
        h.counters.program_changes, h.counters.preview_changes, h.counters.preview_stale_held
    );
    println!(
        "  maxima: cache entries {} (capacity {FRAME_CAPACITY}), ready frames in cache {}, \
         jobs in flight {} (bound {}), composed frames held {}, executor queue {}",
        h.maxima.cache_len,
        h.maxima.ready_in_cache,
        h.maxima.in_flight,
        2 + 2 * PRELOAD_RADIUS,
        h.maxima.composed_held,
        h.maxima.jobs_pending
    );
    println!("  elapsed {:?}", started.elapsed());
}
