//! The Markdown draft workspace (Decision 25): a Markdown or text song opened
//! as an unsaved draft. The file is edited as text; every change is parsed
//! again off the GPUI thread, and the parsed Song, Arrangement, issues and a
//! live preview of the sections follow it. Nothing is written until
//! **Save song**, and then in one transaction through the `LibraryStore`.
//!
//! The shell hosts [`MarkdownDraftView::render_frame`] while the draft is open
//! and replaces it on a [`MarkdownDraftEvent`].

mod issues;
mod preview;
#[cfg(test)]
mod tests;

use domain::{
    identity::{ArrangementId, SongId},
    song_library::{
        PossibleSong,
        import::{MarkdownDraft, SongChoice, markdown},
    },
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, SharedString, Styled, Subscription, Task,
    Window, div, px, rgb,
};
use ui::{
    accessibility::RegionRegistry,
    assets::icon,
    environment::UiEnvironment,
    overlay::{Overlay, OverlayHost},
    primitives::{
        Availability,
        button::{Button, Tone},
        disclosure::DisclosureState,
        text_area::{TextArea, TextAreaEvent, TextAreaState},
    },
    scroll::Scroll,
    tokens,
};

use crate::songs::store::LibraryStore;

/// The F6 regions, in reading order.
const HEADER_ACTIONS: &str = "Workspace actions";
const DETAILS: &str = "Primary list";
const SOURCE: &str = "Main editor";
const PREVIEW: &str = "Inspector";

/// Section labels (Decision 24).
const LYRICS_SOURCE: &str = "Lyrics source";
const UNTITLED: &str = "Untitled song";
const NOT_SAVED: &str = "Not saved yet.";
const DISCARD: &str = "Discard draft";
const SAVE: &str = "Save song";
const BLOCKED: &str = "Fix the blocking issues first.";
const SAVING: &str = "Saving\u{2026}";
const CHECKING: &str = "Checking the draft\u{2026}";
const DISCARD_TITLE: &str = "Discard this draft?";
const KEEP_EDITING: &str = "Keep editing";
const DISCARD_CONFIRM: &str = "Discard";

/// How wide the details and preview columns are, before the Interface scale.
const SIDE_WIDTH: f32 = 320.0;

/// What the draft tells its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownDraftEvent {
    /// The operator left without saving; nothing was written.
    Discarded,
    /// The draft was written; the Arrangement is the one it created.
    Saved(ArrangementId),
}

/// The Markdown draft: the file's text, what it parses to, and where it would
/// be saved. Owned by the shell while open; dropping it discards the draft.
pub struct MarkdownDraftView {
    file: String,
    /// The text the file opened with, to tell an edited draft from an
    /// untouched one.
    original: String,
    source: TextAreaState,
    parsed: MarkdownDraft,
    /// The identities every re-parse uses, so the draft stays one Song and
    /// one Arrangement while it is edited.
    ids: (SongId, ArrangementId),
    /// Which parse is current; an older answer is dropped.
    generation: u64,
    /// Whether the text has changed since `parsed` was made.
    parsing: bool,
    target: SongChoice,
    /// Ranked possible existing Songs for the parsed Song, recomputed when a
    /// parse lands or the library changes, never per frame.
    candidates: Vec<PossibleSong>,
    store: Entity<LibraryStore>,
    regions: RegionRegistry,
    disclosure: DisclosureState,
    details_scroll: Scroll,
    source_scroll: Scroll,
    preview_scroll: Scroll,
    /// The shell's overlay layer, which hosts the nested Song preview and the
    /// Discard confirmation.
    overlays: Option<OverlayHost>,
    /// The open Song preview and the id its host gave it.
    previewed: Option<(usize, SongId)>,
    /// The id of the open Discard confirmation.
    confirming: Option<usize>,
    busy: bool,
    refusal: Option<SharedString>,
    /// The in-flight parse and save, owned here so closing the draft drops the
    /// interest.
    parse_task: Option<Task<()>>,
    save_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<MarkdownDraftEvent> for MarkdownDraftView {}

impl Focusable for MarkdownDraftView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.source.focus_handle(cx)
    }
}

/// The text a file shows: no byte-order mark, line feeds only.
fn display_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.strip_prefix('\u{feff}')
        .unwrap_or(&text)
        .replace("\r\n", "\n")
}

impl MarkdownDraftView {
    /// Opens the draft `draft`, parsed from `bytes` of the file named `file`,
    /// over `store`. Focus starts in the source text.
    pub fn new(
        file: String,
        bytes: Vec<u8>,
        draft: MarkdownDraft,
        store: Entity<LibraryStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = display_text(&bytes);
        let source = TextAreaState::new(window, cx);
        source.set_value(text.clone(), window, cx);
        source.focus(window, cx);
        let weak = cx.weak_entity();
        let mut subscriptions = vec![source.subscribe(cx, move |event, cx| {
            if event == TextAreaEvent::Changed {
                let _ = weak.update(cx, |view, cx| view.edited(cx));
            }
        })];
        subscriptions.push(cx.observe(&store, |view, _, cx| view.recompute(cx)));
        let ids = (draft.document.id, first_arrangement(&draft));
        let mut view = Self {
            file,
            original: text,
            source,
            parsed: draft,
            ids,
            generation: 0,
            parsing: false,
            target: SongChoice::New,
            candidates: Vec::new(),
            store,
            regions: RegionRegistry::default(),
            disclosure: DisclosureState::new(cx),
            details_scroll: Scroll::new(),
            source_scroll: Scroll::new(),
            preview_scroll: Scroll::new(),
            overlays: None,
            previewed: None,
            confirming: None,
            busy: false,
            refusal: None,
            parse_task: None,
            save_task: None,
            _subscriptions: subscriptions,
        };
        view.recompute(cx);
        view
    }

    /// Hands the draft the shell's overlay layer, which hosts the nested Song
    /// preview and the Discard confirmation.
    pub fn set_overlays(&mut self, overlays: OverlayHost) {
        self.overlays = Some(overlays);
    }

    /// What the file's current text parses to. While a re-parse is in flight
    /// this is the last finished one.
    pub fn parsed(&self) -> &MarkdownDraft {
        &self.parsed
    }

    /// Whether **Save song** can be taken: the current text has parsed, it has
    /// no blocking issue, and no save is in flight.
    pub fn can_save(&self) -> bool {
        !self.parsing && !self.busy && self.parsed.can_save()
    }

    /// The source text field, so a test or lab can type into it.
    pub fn source(&self) -> &TextAreaState {
        &self.source
    }

    /// The Song the draft would be saved under.
    pub fn target(&self) -> SongChoice {
        self.target
    }

    /// The possible existing Songs for the parsed Song, strongest first.
    pub fn candidates(&self) -> &[PossibleSong] {
        &self.candidates
    }

    /// The reason the last save was refused, while the header shows it.
    pub fn refusal(&self) -> Option<&SharedString> {
        self.refusal.as_ref()
    }

    /// Whether a save is in flight.
    pub fn busy(&self) -> bool {
        self.busy
    }

    /// Whether a re-parse of the latest text is still pending.
    pub fn parsing(&self) -> bool {
        self.parsing
    }

    /// Whether the text differs from what the file held.
    pub fn has_edits(&self, cx: &App) -> bool {
        self.source.value(cx).as_ref() != self.original
    }

    /// A handler for a control, run against this entity.
    pub(super) fn act(
        cx: &Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl Fn(&mut Window, &mut App) + 'static {
        let weak = cx.weak_entity();
        move |window, cx| {
            let _ = weak.update(cx, |view, cx| run(view, window, cx));
        }
    }

    /// The text changed: parses it again off the GPUI thread. Only the newest
    /// answer is applied.
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.parsing = true;
        self.refusal = None;
        let text = self.source.value(cx).to_string();
        let ids = self.ids;
        let parse = cx.background_spawn(async move { markdown::parse(text.as_bytes(), ids) });
        self.parse_task = Some(cx.spawn(async move |this, cx| {
            let draft = parse.await;
            let _ = this.update(cx, |view, cx| view.reparsed(generation, draft, cx));
        }));
        cx.notify();
    }

    fn reparsed(&mut self, generation: u64, draft: MarkdownDraft, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        self.parsed = draft;
        self.parsing = false;
        self.parse_task = None;
        self.recompute(cx);
    }

    /// Ranks the possible existing Songs for the parsed Song.
    ///
    /// This runs on the GPUI thread, as the Library snapshot it ranks against
    /// lives there; the shaping constraint is that a parse lands once per edit
    /// burst, not per frame. T2.10c's 16 ms timing check covers it with a
    /// Library of realistic size; if it fails, the ranking moves to a snapshot
    /// in the background like the import classification does.
    fn recompute(&mut self, cx: &mut Context<Self>) {
        self.candidates = self
            .store
            .read(cx)
            .library()
            .possible_existing_songs(&self.parsed.document.song);
        if self.candidates.is_empty() {
            // Collapsed, so it does not reappear already expanded.
            self.disclosure.set_expanded(false, cx);
        }
        cx.notify();
    }

    /// Moves the caret to the start of 1-based `line` and focuses the text.
    pub(super) fn go_to_line(&mut self, line: usize, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.source.value(cx);
        let at = issues::line_start(&text, line);
        self.source.set_selected_range(at..at, cx);
        self.source.focus(window, cx);
        cx.notify();
    }

    /// Opens the nested, identity-only preview of an existing Song.
    pub(super) fn preview_song(
        &mut self,
        song: SongId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(host), None, false) = (self.overlays.clone(), self.previewed, self.busy) else {
            return;
        };
        let Some(overlay) = preview::overlay(self, song, cx) else {
            return;
        };
        let id = host.open(overlay, window, cx);
        self.previewed = Some((id, song));
        cx.notify();
    }

    /// The nested preview's surface has gone, however it was dismissed.
    pub(super) fn preview_closed(&mut self, cx: &mut Context<Self>) {
        self.previewed = None;
        cx.notify();
    }

    /// **Use this existing Song**: the draft will be saved as one more
    /// Arrangement of `song`.
    pub(super) fn use_existing_song(&mut self, song: SongId, cx: &mut Context<Self>) {
        if self.store.read(cx).library().song(song).is_ok() {
            self.target = SongChoice::Existing(song);
            self.refusal = None;
            cx.notify();
        }
    }

    /// **Add as a new Song**: the draft will be saved as a new Song again.
    pub(super) fn use_new_song(&mut self, cx: &mut Context<Self>) {
        self.target = SongChoice::New;
        self.refusal = None;
        cx.notify();
    }

    /// **Save song**: writes the Song and Arrangement in one transaction. On
    /// success the draft emits [`MarkdownDraftEvent::Saved`]; a refusal stays
    /// in the header and the draft stays open.
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save() {
            return;
        }
        self.busy = true;
        self.refusal = None;
        let draft = self.parsed.clone();
        let target = self.target;
        let task = self
            .store
            .update(cx, |store, cx| store.save_markdown(draft, target, cx));
        let weak = cx.weak_entity();
        self.save_task = Some(window.spawn(cx, async move |cx| {
            let result = task.await;
            cx.update(|window, cx| {
                // Close what this view opened first, outside its own update:
                // closing hands focus back and calls into the view.
                let open = weak.read_with(cx, |view, _| {
                    (view.overlays.clone(), view.previewed, view.confirming)
                });
                if let Ok((Some(host), previewed, confirming)) = open {
                    for id in previewed.map(|(id, _)| id).into_iter().chain(confirming) {
                        host.close(id, window, cx);
                    }
                }
                let _ = weak.update(cx, |view, cx| {
                    view.busy = false;
                    view.save_task = None;
                    match result {
                        Ok(arrangement) => cx.emit(MarkdownDraftEvent::Saved(arrangement)),
                        Err(message) => view.refusal = Some(message),
                    }
                    cx.notify();
                });
            })
            .ok();
        }));
        cx.notify();
    }

    /// **Discard draft**: closes at once when nothing was edited, and asks
    /// first when something was.
    pub fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(host) = self.overlays.clone() else {
            cx.emit(MarkdownDraftEvent::Discarded);
            return;
        };
        if !self.has_edits(cx) {
            cx.emit(MarkdownDraftEvent::Discarded);
            return;
        }
        if self.confirming.is_some() {
            return;
        }
        let confirm = cx.weak_entity();
        let closed = cx.weak_entity();
        let overlay = Overlay::dialog(DISCARD_TITLE)
            .description(format!("Nothing from {} has been saved.", self.file))
            .primary(DISCARD_CONFIRM, move |_, cx| {
                let _ = confirm.update(cx, |_, cx| cx.emit(MarkdownDraftEvent::Discarded));
            })
            .primary_tone(Tone::Danger)
            .secondary(KEEP_EDITING, |_, _| {})
            .on_close(move |_, cx| {
                let _ = closed.update(cx, |view, cx| {
                    view.confirming = None;
                    cx.notify();
                });
            });
        self.confirming = Some(host.open(overlay, window, cx));
        cx.notify();
    }

    /// The title the header shows.
    fn title(&self) -> String {
        let title = self.parsed.document.song.title.trim();
        if title.is_empty() {
            UNTITLED.to_owned()
        } else {
            title.to_owned()
        }
    }

    /// The whole frame: the header, then the details, the source with its
    /// issues and the preview, as four F6 regions in that order.
    pub fn render_frame(
        &mut self,
        environment: UiEnvironment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        let header = self.regions.region(HEADER_ACTIONS, window, cx);
        let header = self.header(header, environment, cx);
        let details = self.regions.region(DETAILS, window, cx);
        let details = column(
            details,
            &self.details_scroll,
            "markdown-details-scroll",
            preview::details(self, environment, cx),
            environment,
            window,
            cx,
        );
        let main = self.regions.region(SOURCE, window, cx);
        let main = column(
            main,
            &self.source_scroll,
            "markdown-source-scroll",
            self.source_pane(environment, cx),
            environment,
            window,
            cx,
        );
        let inspector = self.regions.region(PREVIEW, window, cx);
        let inspector = column(
            inspector,
            &self.preview_scroll,
            "markdown-preview-scroll",
            preview::render(self, environment),
            environment,
            window,
            cx,
        );
        let border = |column: gpui::Div| {
            column
                .h_full()
                .border_color(rgb(palette.border))
                .border_r(px(tokens::border::SEPARATOR))
        };
        self.regions
            .root(window, cx)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.background))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(border(
                        div()
                            .flex_none()
                            .w(px(scale.apply(SIDE_WIDTH)))
                            .child(details),
                    ))
                    .child(border(div().flex_1().min_w_0().child(main)))
                    .child(
                        div()
                            .flex_none()
                            .w(px(scale.apply(SIDE_WIDTH)))
                            .h_full()
                            .child(inspector),
                    ),
            )
            .into_any_element()
    }

    /// The region that currently holds focus, for tests.
    pub fn focused_region(&self, window: &Window, cx: &App) -> Option<SharedString> {
        self.regions.focused_region(window, cx)
    }

    fn header(
        &self,
        region: gpui::Stateful<gpui::Div>,
        environment: UiEnvironment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        // Unavailable, not Disabled, and the label stays: the button keeps its
        // place in the tab order, so focus is not lost while it waits.
        let availability = if self.busy {
            Availability::unavailable(SAVING)
        } else if self.parsing {
            Availability::unavailable(CHECKING)
        } else if self.parsed.can_save() {
            Availability::Available
        } else {
            Availability::unavailable(BLOCKED)
        };
        let label = match self.target {
            SongChoice::Existing(song) => self.store.read(cx).library().song(song).map_or_else(
                |_| SAVE.to_owned(),
                |work| format!("Save to {}", work.title()),
            ),
            _ => SAVE.to_owned(),
        };
        let save = Button::new("markdown-save", label, environment)
            .icon(icon::CHECK)
            .tone(Tone::Primary)
            .on_surface(palette.surface)
            .availability(availability)
            .on_activate(Self::act(cx, |view, window, cx| view.save(window, cx)));
        region
            .flex_none()
            .flex()
            .items_center()
            .gap(px(scale.apply(tokens::spacing::RELATED)))
            .px(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .py(px(scale.apply(tokens::spacing::RELATED)))
            .bg(rgb(palette.surface))
            .border_b(px(tokens::border::SEPARATOR))
            .border_color(rgb(palette.border))
            .child(
                Button::new("markdown-discard", DISCARD, environment)
                    .icon(icon::X)
                    .icon_only()
                    .quiet()
                    .tooltip(DISCARD)
                    .on_surface(palette.surface)
                    .availability(if self.busy {
                        Availability::unavailable(SAVING)
                    } else {
                        Availability::Available
                    })
                    .on_activate(Self::act(cx, |view, window, cx| view.discard(window, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(scale.apply(tokens::typography::META.size)))
                            .text_color(rgb(palette.text_muted))
                            .child(format!("Library / Songs \u{b7} {}", self.file)),
                    )
                    .child(
                        div()
                            .debug_selector(|| "markdown-title".into())
                            .text_size(px(scale.apply(tokens::typography::HEADING.size)))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.text))
                            .child(self.title()),
                    ),
            )
            .child(
                div()
                    .text_size(px(scale.apply(tokens::typography::META.size)))
                    .text_color(rgb(palette.text_muted))
                    .child(NOT_SAVED),
            )
            .children(self.refusal.clone().map(|refusal| {
                div()
                    .debug_selector(|| "markdown-refusal".into())
                    .text_color(rgb(palette.danger))
                    .child(refusal)
            }))
            .child(save)
            .into_any_element()
    }

    /// The middle column: the text, then its issues.
    fn source_pane(&self, environment: UiEnvironment, cx: &mut Context<Self>) -> AnyElement {
        let palette = tokens::palette(environment.appearance());
        let scale = environment.preferences().scale;
        div()
            .flex()
            .flex_col()
            .gap(px(scale.apply(tokens::spacing::GROUP)))
            .w_full()
            .p(px(scale.apply(tokens::spacing::CONTROL_INSET)))
            .child(
                div().debug_selector(|| "markdown-source".into()).child(
                    TextArea::new(
                        "markdown-source-area",
                        LYRICS_SOURCE,
                        &self.source,
                        environment,
                    )
                    .rows(12, 28)
                    .on_surface(palette.background),
                ),
            )
            .child(issues::render(self, environment, cx))
            .into_any_element()
    }
}

/// One scrolling column of the frame.
fn column(
    region: gpui::Stateful<gpui::Div>,
    scroll: &Scroll,
    id: &'static str,
    body: AnyElement,
    environment: UiEnvironment,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    region
        .size_full()
        .relative()
        .child(scroll.bounded(div().id(id)).size_full().child(body))
        .children(scroll.indicator(id, 0.0, window, cx, environment))
        .into_any_element()
}

/// The Arrangement the draft carries.
///
/// The parser always makes exactly one; if a draft ever came without, a random
/// id keeps the draft usable rather than colliding with a real one.
fn first_arrangement(draft: &MarkdownDraft) -> ArrangementId {
    draft.document.arrangements.first().map_or_else(
        || {
            ArrangementId::from_entity(domain::identity::EntityId::from_random_bytes(
                *uuid::Uuid::new_v4().as_bytes(),
            ))
        },
        |a| a.id,
    )
}
