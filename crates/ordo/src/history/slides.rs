//! The Slides tab's frames: the canonical frame of every slide of one side,
//! composed off the GPUI thread and cached by (section name, slide body), the
//! way the Lyrics navigator caches its thumbnails.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use domain::{
    pagination::{slide_body, slides},
    song_library::ArrangementLyrics,
};
use gpui::{AnyElement, Context, IntoElement, Styled, Task, div, px, rgb};
use rendering::{FrameCache, FrameEntry, TemplateContent, TemplateKind};
use ui::{environment::UiEnvironment, tokens};

use crate::canonical_views::live_viewport;

/// What a composed frame is cached under: the section name and the slide's
/// rendered body.
pub type FrameKey = (Option<String>, String);

/// The most frames held at once, as the Lyrics editor's cache.
pub const CAPACITY: usize = 512;

/// The frames the Slides tab shows, and the composition still in flight.
pub struct SlideFrames {
    frames: FrameCache<FrameKey>,
    pending: HashMap<FrameKey, Task<()>>,
}

impl SlideFrames {
    /// An empty cache.
    pub fn new() -> Self {
        Self {
            frames: FrameCache::new(CAPACITY),
            pending: HashMap::new(),
        }
    }

    /// Frames composed or failed, held now.
    pub fn held(&self) -> usize {
        self.frames.len()
    }

    /// Frames being composed now.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Starts composing the wanted frames that nothing holds yet. Composition
    /// runs on the background executor; the request is cut at the cache's
    /// capacity so it never evicts its own members.
    pub fn request(&mut self, wanted: Vec<FrameKey>, cx: &mut Context<Self>) {
        let mut seen = HashSet::new();
        let wanted: Vec<FrameKey> = wanted
            .into_iter()
            .filter(|key| seen.insert(key.clone()))
            .take(CAPACITY)
            .collect();
        for key in wanted {
            if self.frames.get(&key).is_some() || self.pending.contains_key(&key) {
                continue;
            }
            let content = TemplateContent {
                kind: TemplateKind::Lyrics,
                header: key.0.clone(),
                body: key.1.clone(),
                footer: None,
                image: None,
            };
            let stored = key.clone();
            let task = cx.spawn(async move |frames, cx| {
                let composed = cx
                    .background_executor()
                    .spawn(async move { rendering::compose(&content) })
                    .await;
                let _ = frames.update(cx, |frames, cx| {
                    frames.pending.remove(&stored);
                    frames.frames.insert(stored, composed.map(Arc::new));
                    cx.notify();
                });
            });
            self.pending.insert(key, task);
        }
    }

    /// A frame box of `width`: the composed frame when it is ready, else an
    /// empty box of the same size.
    pub fn picture(&self, key: &FrameKey, width: f32, environment: UiEnvironment) -> AnyElement {
        match self.frames.peek(key) {
            Some(FrameEntry::Ready(frame)) => {
                live_viewport(Some(frame.clone()), None, width, None, environment)
            }
            _ => div()
                .w(px(width))
                .h(px(width * 9.0 / 16.0))
                .rounded(px(tokens::radius::SLIDE_VIEWPORT))
                .bg(rgb(
                    tokens::palette(environment.appearance()).surface_secondary
                ))
                .into_any_element(),
        }
    }
}

impl Default for SlideFrames {
    fn default() -> Self {
        Self::new()
    }
}

/// One slide of the Sequence, in sung order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SungSlide {
    /// The zero-based position of its occurrence.
    pub position: usize,
    /// The section's name.
    pub name: String,
    /// Where the frame is cached.
    pub key: FrameKey,
}

/// Every slide of `lyrics` in sung order.
pub fn sung_slides(lyrics: &ArrangementLyrics) -> Vec<SungSlide> {
    let mut sung = Vec::new();
    for (position, occurrence) in lyrics.sequence.iter().enumerate() {
        let Some(section) = lyrics
            .sections
            .iter()
            .find(|section| section.id == occurrence.section)
        else {
            continue;
        };
        for slide in slides(&section.lyrics) {
            sung.push(SungSlide {
                position,
                name: section
                    .label
                    .clone()
                    .unwrap_or_else(|| "Unnamed section".to_owned()),
                key: (section.label.clone(), slide_body(&section.lyrics, &slide)),
            });
        }
    }
    sung
}
