//! Compile-time catalog of Ordo's bundled operator assets.

pub const ATKINSON_HYPERLEGIBLE_NEXT: &[u8] =
    include_bytes!("../assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf");
pub const NOTO_SANS: &[u8] = include_bytes!("../assets/fonts/noto-sans/NotoSans[wdth,wght].ttf");

pub mod icon {
    pub const ACTIVITY: &str = include_str!("../assets/icons/lucide/activity.svg");
    pub const CALENDAR_DAYS: &str = include_str!("../assets/icons/lucide/calendar-days.svg");
    pub const CHECK: &str = include_str!("../assets/icons/lucide/check.svg");
    pub const CHEVRON_DOWN: &str = include_str!("../assets/icons/lucide/chevron-down.svg");
    pub const CHEVRON_RIGHT: &str = include_str!("../assets/icons/lucide/chevron-right.svg");
    pub const CIRCLE_ALERT: &str = include_str!("../assets/icons/lucide/circle-alert.svg");
    pub const CIRCLE_QUESTION_MARK: &str =
        include_str!("../assets/icons/lucide/circle-question-mark.svg");
    pub const DATABASE_BACKUP: &str = include_str!("../assets/icons/lucide/database-backup.svg");
    pub const ELLIPSIS: &str = include_str!("../assets/icons/lucide/ellipsis.svg");
    pub const GRIP_VERTICAL: &str = include_str!("../assets/icons/lucide/grip-vertical.svg");
    pub const INFO: &str = include_str!("../assets/icons/lucide/info.svg");
    pub const LIBRARY: &str = include_str!("../assets/icons/lucide/library.svg");
    pub const SETTINGS: &str = include_str!("../assets/icons/lucide/settings.svg");
    pub const SWATCH_BOOK: &str = include_str!("../assets/icons/lucide/swatch-book.svg");
    pub const TRIANGLE_ALERT: &str = include_str!("../assets/icons/lucide/triangle-alert.svg");
    pub const X: &str = include_str!("../assets/icons/lucide/x.svg");
}
