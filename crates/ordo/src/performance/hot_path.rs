//! GPUI-thread hot-path violation counting for T0.6 WP5.
//!
//! The keydown-to-paint path must never reach storage, the filesystem, a
//! worker wait, or a full-timeline rerender. Those choke points call [`note`]
//! with the kind of work they are about to do. A note made while a scope from
//! [`enter`] is open is a violation and is counted per kind. A note made with
//! no scope open is ignored, so off-path work (idle saves, shutdown sends)
//! costs nothing.
//!
//! State is thread-local. The scope and the notes belong to the GPUI thread,
//! which is the only thread the keydown-to-paint path runs on. [`HotPathScope`]
//! is deliberately `!Send`, so a scope cannot be closed on another thread.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    marker::PhantomData,
};

/// Kinds of off-path work that must not run between keydown and paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HotPathKind {
    /// A message sent to a background worker and possibly waited on.
    WorkerSend,
    /// A preference write, which is a storage and filesystem operation.
    PreferenceSave,
    /// Direct filesystem or database access.
    FileAccess,
    /// A full canonical recomposition of a timeline.
    CanonicalRecompose,
    /// PNG encode or decode, or construction of an image from encoded bytes.
    /// Production sites: the canonical viewport finish and the congregation
    /// image. Decode happens inside GPUI at paint time and is not noted.
    ImageDecode,
    /// Pagination of a timeline. No production code exists yet (T1 scope);
    /// the kind is reserved so that work is counted when it lands.
    Pagination,
    /// Autosave of the operator document. No production code exists yet; the
    /// kind is reserved so that work is counted when it lands.
    Autosave,
}

impl HotPathKind {
    /// Every kind, in report order.
    pub const ALL: [HotPathKind; 7] = [
        HotPathKind::WorkerSend,
        HotPathKind::PreferenceSave,
        HotPathKind::FileAccess,
        HotPathKind::CanonicalRecompose,
        HotPathKind::ImageDecode,
        HotPathKind::Pagination,
        HotPathKind::Autosave,
    ];

    /// Stable lowercase name used in reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            HotPathKind::WorkerSend => "worker_send",
            HotPathKind::PreferenceSave => "preference_save",
            HotPathKind::FileAccess => "file_access",
            HotPathKind::CanonicalRecompose => "canonical_recompose",
            HotPathKind::ImageDecode => "image_decode",
            HotPathKind::Pagination => "pagination",
            HotPathKind::Autosave => "autosave",
        }
    }
}

thread_local! {
    /// Number of scopes currently open on this thread.
    static OPEN_SCOPES: Cell<usize> = const { Cell::new(0) };
    /// Violations counted on this thread, per kind.
    static VIOLATIONS: RefCell<BTreeMap<HotPathKind, u64>> = const { RefCell::new(BTreeMap::new()) };
}

/// An open hot-path scope. The scope ends when this guard is dropped or
/// [`HotPathScope::close`] is called. Scopes nest, and notes count while any
/// one of them is open.
#[must_use = "the hot-path scope ends as soon as the guard is dropped"]
#[derive(Debug)]
pub struct HotPathScope {
    /// Thread-local state must be closed on the thread that opened it.
    _not_send: PhantomData<*const ()>,
}

impl HotPathScope {
    /// Ends this scope. Equivalent to dropping it, but reads as an intent at
    /// the call site.
    pub fn close(self) {}
}

impl Drop for HotPathScope {
    fn drop(&mut self) {
        OPEN_SCOPES.with(|open| open.set(open.get().saturating_sub(1)));
    }
}

/// Opens a hot-path scope on the current thread.
pub fn enter() -> HotPathScope {
    OPEN_SCOPES.with(|open| open.set(open.get().saturating_add(1)));
    HotPathScope {
        _not_send: PhantomData,
    }
}

/// Whether any scope is open on the current thread.
pub fn is_open() -> bool {
    OPEN_SCOPES.with(|open| open.get() > 0)
}

/// Records that `kind` of off-path work is about to run. Counted as a
/// violation only while a scope is open on this thread.
pub fn note(kind: HotPathKind) {
    if is_open() {
        VIOLATIONS.with(|violations| {
            *violations.borrow_mut().entry(kind).or_insert(0) += 1;
        });
    }
}

/// Violations counted on this thread so far, per kind, in report order.
/// Kinds with no violations are omitted.
pub fn violations() -> Vec<(HotPathKind, u64)> {
    VIOLATIONS.with(|violations| {
        violations
            .borrow()
            .iter()
            .map(|(kind, count)| (*kind, *count))
            .collect()
    })
}

/// Returns the violations counted on this thread and resets the counters.
pub fn take_violations() -> Vec<(HotPathKind, u64)> {
    let taken = VIOLATIONS.with(|violations| std::mem::take(&mut *violations.borrow_mut()));
    taken.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_inside_a_scope_is_counted() {
        take_violations();
        let scope = enter();
        assert!(is_open());
        note(HotPathKind::WorkerSend);
        note(HotPathKind::WorkerSend);
        note(HotPathKind::PreferenceSave);
        scope.close();
        assert_eq!(
            take_violations(),
            vec![
                (HotPathKind::WorkerSend, 2),
                (HotPathKind::PreferenceSave, 1)
            ]
        );
    }

    #[test]
    fn note_outside_a_scope_is_not_counted() {
        take_violations();
        note(HotPathKind::FileAccess);
        assert!(!is_open());
        assert!(violations().is_empty());
    }

    #[test]
    fn note_after_the_scope_closes_is_not_counted() {
        take_violations();
        {
            let _scope = enter();
            note(HotPathKind::CanonicalRecompose);
        }
        note(HotPathKind::CanonicalRecompose);
        assert_eq!(
            take_violations(),
            vec![(HotPathKind::CanonicalRecompose, 1)]
        );
    }

    #[test]
    fn nested_scopes_count_until_the_outermost_closes() {
        take_violations();
        let outer = enter();
        let inner = enter();
        inner.close();
        assert!(is_open());
        note(HotPathKind::FileAccess);
        outer.close();
        assert!(!is_open());
        assert_eq!(violations(), vec![(HotPathKind::FileAccess, 1)]);
        take_violations();
    }

    #[test]
    fn take_resets_the_counters() {
        take_violations();
        let _scope = enter();
        note(HotPathKind::WorkerSend);
        assert_eq!(take_violations(), vec![(HotPathKind::WorkerSend, 1)]);
        assert!(violations().is_empty());
    }
}
