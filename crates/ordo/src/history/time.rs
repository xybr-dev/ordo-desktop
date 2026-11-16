//! The times History shows, as pure functions of a timestamp, the current
//! moment and the local offset (Decision 21). Nothing here reads a clock.

use domain::identity::Timestamp;
use time::{OffsetDateTime, UtcOffset};

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The moment of `at` on the wall clock of `offset`.
fn local(at: Timestamp, offset: UtcOffset) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(at.millis()) * 1_000_000)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .to_offset(offset)
}

/// The local offset, or UTC when the platform cannot say. Read on the GPUI
/// thread only, as `recovery` does.
pub fn local_offset() -> UtcOffset {
    UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)
}

/// The time of day, `10:44`, on a 24-hour clock.
pub fn clock(at: Timestamp, offset: UtcOffset) -> String {
    let local = local(at, offset);
    format!("{:02}:{:02}", local.hour(), local.minute())
}

/// The day: `Today`, `Yesterday`, or `14 Aug 2026`.
pub fn day(at: Timestamp, now: OffsetDateTime, offset: UtcOffset) -> String {
    let local = local(at, offset);
    let today = now.to_offset(offset).date();
    if local.date() == today {
        "Today".to_owned()
    } else if today.previous_day() == Some(local.date()) {
        "Yesterday".to_owned()
    } else {
        format!(
            "{} {} {}",
            local.day(),
            MONTHS[usize::from(u8::from(local.month())) - 1],
            local.year()
        )
    }
}

/// The day and the time of day together, `Yesterday 16:12`.
pub fn stamp(at: Timestamp, now: OffsetDateTime, offset: UtcOffset) -> String {
    format!("{} {}", day(at, now, offset), clock(at, offset))
}

/// A revision's time in a sentence: the time of day for today, else the day
/// and the time (`Yesterday 16:12`), so an older revision is never ambiguous.
pub fn when(at: Timestamp, now: OffsetDateTime, offset: UtcOffset) -> String {
    if day(at, now, offset) == "Today" {
        clock(at, offset)
    } else {
        stamp(at, now, offset)
    }
}

/// A session's time: `Today \u{b7} 10:37\u{2013}10:44`, or one time when it began and
/// ended in the same minute. A session that crosses midnight names the day it
/// began.
pub fn session_when(
    first: Timestamp,
    last: Timestamp,
    now: OffsetDateTime,
    offset: UtcOffset,
) -> String {
    let (begin, end) = (clock(first, offset), clock(last, offset));
    let day = day(first, now, offset);
    if begin == end {
        format!("{day} \u{b7} {begin}")
    } else {
        format!("{day} \u{b7} {begin}\u{2013}{end}")
    }
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::*;

    fn at(day: u8, hour: u8, minute: u8, offset: UtcOffset) -> Timestamp {
        let date = Date::from_calendar_date(2026, Month::August, day).expect("date");
        let moment = date
            .with_hms(hour, minute, 0)
            .expect("time")
            .assume_offset(offset);
        Timestamp::from_millis(moment.unix_timestamp() * 1_000)
    }

    fn now(offset: UtcOffset) -> OffsetDateTime {
        Date::from_calendar_date(2026, Month::August, 20)
            .expect("date")
            .with_hms(12, 0, 0)
            .expect("time")
            .assume_offset(offset)
    }

    #[test]
    fn a_session_reads_today_yesterday_or_its_date() {
        let utc = UtcOffset::UTC;
        let now = now(utc);
        assert_eq!(
            session_when(at(20, 10, 37, utc), at(20, 10, 44, utc), now, utc),
            "Today \u{b7} 10:37\u{2013}10:44"
        );
        assert_eq!(
            session_when(at(19, 16, 12, utc), at(19, 16, 12, utc), now, utc),
            "Yesterday \u{b7} 16:12"
        );
        assert_eq!(
            session_when(at(14, 9, 8, utc), at(14, 9, 8, utc), now, utc),
            "14 Aug 2026 \u{b7} 09:08"
        );
        assert_eq!(stamp(at(19, 16, 12, utc), now, utc), "Yesterday 16:12");
    }

    #[test]
    fn the_local_offset_decides_the_day_and_the_hour() {
        let manila = UtcOffset::from_hms(8, 0, 0).expect("offset");
        let utc = UtcOffset::UTC;
        // 23:30 UTC on the 19th is 07:30 on the 20th in Manila.
        let late = at(19, 23, 30, utc);
        assert_eq!(clock(late, manila), "07:30");
        assert_eq!(day(late, now(manila), manila), "Today");
        assert_eq!(day(late, now(utc), utc), "Yesterday");
    }
}
