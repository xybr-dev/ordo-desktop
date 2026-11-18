//! The review's evidence in the operator's words (Decision 24). Pure: the
//! domain classifies, this composes the sentences the review shows.

use domain::{
    identity::ArrangementId,
    song_library::{
        MatchedOn, SongLibrary, SongWork,
        import::{
            ArrangementDifference, ArrangementEvidence, ArrangementReview, DetailField,
            ImportDocument, InvalidReason, SongEvidence, SongIdentity, Tally,
        },
    },
};

/// "a", "a and b", "a, b, and c".
pub(super) fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// "a", "a and b", "a, b and c": the plain list a Song's matching aspects
/// read as, where [join_and] is for lists of differences.
pub(super) fn join_plain(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect::<String>()
    })
}

fn field_name(field: DetailField) -> &'static str {
    match field {
        DetailField::Title => "title",
        DetailField::Languages => "languages",
        DetailField::Choirs => "Choirs",
        DetailField::RenditionAttribution => "translator or adapter credit",
        DetailField::Occasions => "occasions",
        DetailField::Notes => "notes",
    }
}

fn difference(difference: &ArrangementDifference) -> String {
    match difference {
        ArrangementDifference::Details(fields) => {
            let names: Vec<String> = fields
                .iter()
                .map(|field| field_name(*field).to_owned())
                .collect();
            let verb = if names.len() == 1 {
                "differs"
            } else {
                "differ"
            };
            format!("the {} {verb}", join_and(&names))
        }
        ArrangementDifference::SectionReworded(name) => format!("{name} is reworded"),
        ArrangementDifference::SlideBreaks(name) => format!("{name} has different slide breaks"),
        ArrangementDifference::SectionAdded(name) => format!("a {name} is added"),
        ArrangementDifference::SectionRemoved(name) => format!("{name} is removed"),
        ArrangementDifference::SequenceChanged => "the Sequence differs".to_owned(),
        ArrangementDifference::SectionsReordered => {
            "the sections are in a different order".to_owned()
        }
    }
}

fn differences(list: &[ArrangementDifference]) -> String {
    join_and(&list.iter().map(difference).collect::<Vec<_>>())
}

/// Why a Song is, or may be, the document's Song.
fn song_why(
    evidence: &[SongEvidence],
    document: &ImportDocument,
    song: Option<&SongWork>,
) -> String {
    let title = song.map_or("this Song", SongWork::title);
    let mut aspects: Vec<String> = Vec::new();
    let mut aliases: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for item in evidence {
        match item {
            SongEvidence::SameId => {
                lines.push("The file names this Song by its identity.".to_owned());
            }
            SongEvidence::SameArrangementId => {
                lines.push("It contains an Arrangement the Library already has.".to_owned());
            }
            SongEvidence::Name {
                candidate_on,
                existing_on,
                ..
            } => {
                // A matching alias is named on its own, not listed among the
                // aspects: "the alias X" reads as a name, not as a field.
                let (aspect, alias) = match (candidate_on, existing_on) {
                    (MatchedOn::Title, MatchedOn::Title) => ("title".to_owned(), None),
                    (_, MatchedOn::Alias(at)) => (
                        "alias".to_owned(),
                        song.and_then(|song| song.aliases().get(*at)).cloned(),
                    ),
                    (MatchedOn::Alias(at), _) => (
                        "alias".to_owned(),
                        document.song.aliases.get(*at).cloned(),
                    ),
                };
                match alias {
                    Some(alias) => {
                        if !aliases.contains(&alias) {
                            aliases.push(alias);
                        }
                    }
                    None => {
                        if !aspects.contains(&aspect) {
                            aspects.push(aspect);
                        }
                    }
                }
            }
            SongEvidence::SameAttribution => aspects.push("composer".to_owned()),
            SongEvidence::SameOpeningLine => aspects.push("opening line".to_owned()),
            SongEvidence::FileTitle(name) => lines.push(format!("The file names it {name}.")),
            SongEvidence::AlsoCalled(alias) => {
                lines.push(format!("The file also calls it {alias}."));
            }
        }
    }
    let mut sentences = Vec::new();
    let alias_clause = |lead: &str| {
        let (noun, verb) = if aliases.len() == 1 {
            ("alias", "matches")
        } else {
            ("aliases", "match")
        };
        (!aliases.is_empty()).then(|| format!("{lead} {noun} {} {verb}", join_plain(&aliases)))
    };
    match (aspects.is_empty(), aliases.is_empty()) {
        (false, true) => {
            let verb = if aspects.len() == 1 { "matches" } else { "match" };
            sentences.push(format!(
                "{} {verb} {title}.",
                capitalised(&join_plain(&aspects))
            ));
        }
        (false, false) => {
            let verb = if aspects.len() == 1 { "matches" } else { "match" };
            let also = if aliases.len() == 1 { "matches" } else { "match" };
            sentences.push(format!(
                "{} {verb} {title}; the {} {} also {also}.",
                capitalised(&join_plain(&aspects)),
                if aliases.len() == 1 { "alias" } else { "aliases" },
                join_plain(&aliases),
            ));
        }
        (true, false) => {
            if let Some(clause) = alias_clause("The") {
                sentences.push(format!("{clause} {title}."));
            }
        }
        (true, true) => {}
    }
    sentences.extend(lines);
    sentences.join(" ")
}

/// The bold lead and the reason under it, for the Song a document belongs to.
/// An identity that still needs a decision has no sentence: the review shows
/// the identity block instead.
pub fn song_sentence(
    identity: &SongIdentity,
    document: &ImportDocument,
    library: &SongLibrary,
) -> (&'static str, String) {
    match identity {
        SongIdentity::Existing { song, evidence } => (
            "Same Song work.",
            song_why(evidence, document, library.song(*song).ok()),
        ),
        SongIdentity::New => (
            "No existing Song.",
            "No Song in the Library matches the title, composer, aliases or opening line."
                .to_owned(),
        ),
        SongIdentity::StagedBy { .. } => (
            "Same Song work.",
            "An earlier document in this batch adds it.".to_owned(),
        ),
        SongIdentity::NeedsDecision { .. } => ("No existing Song.", String::new()),
    }
}

/// Why a document is the same Song as an earlier one of the batch.
pub fn staged_by_sentence(file: &str) -> String {
    format!("Same Song work as {file}")
}

fn label_of(library: &SongLibrary, id: Option<ArrangementId>) -> String {
    id.and_then(|id| library.arrangement_label(id).ok())
        .unwrap_or_else(|| "existing".to_owned())
}

/// Why an Arrangement was classified as it was, in one paragraph. Extraction
/// warnings are not part of it: they are listed as notes from the file.
pub fn arrangement_sentence(review: &ArrangementReview, library: &SongLibrary) -> String {
    let label = label_of(library, review.nearest);
    let same_id = review
        .evidence
        .iter()
        .any(|item| matches!(item, ArrangementEvidence::SameId));
    let mut sentences: Vec<String> = Vec::new();
    for item in &review.evidence {
        match item {
            ArrangementEvidence::SameId | ArrangementEvidence::FileWarning(_) => {}
            ArrangementEvidence::Identical => sentences.push(format!(
                "Identical to the existing {label} Arrangement: lyrics, Sequence and slide breaks all match."
            )),
            ArrangementEvidence::Resembles(list) if same_id => sentences.push(format!(
                "Same Arrangement as {label}, changed since: {}.",
                differences(list)
            )),
            ArrangementEvidence::Resembles(list) => sentences.push(format!(
                "Resembles the existing {label} Arrangement: {}.",
                differences(list)
            )),
            ArrangementEvidence::NoArrangementForChoir(name) => {
                sentences.push(format!("Choir {name} has no Arrangement of this Song."));
            }
            ArrangementEvidence::NewLanguage(language) => {
                sentences.push(format!("A new {language} rendition of the same composition."));
            }
            ArrangementEvidence::NoMatchingArrangement => {
                sentences.push("No existing Arrangement resembles it.".to_owned());
            }
            ArrangementEvidence::CreatesSong { arrangements: 1 | 0 } => {
                sentences.push("Creates the Song work and its first Arrangement.".to_owned());
            }
            ArrangementEvidence::CreatesSong { arrangements } => sentences.push(format!(
                "Creates the Song work and its {arrangements} Arrangements."
            )),
            ArrangementEvidence::AddsToStagedSong => {
                sentences.push("Adds to a Song an earlier document of this batch creates.".to_owned());
            }
            ArrangementEvidence::AddsChoir(name) => {
                sentences.push(format!("Adds the Choir {name}."));
            }
            ArrangementEvidence::ArchivedChoir(name) => {
                sentences.push(format!("{name} is archived, so it won't be linked."));
            }
            ArrangementEvidence::ExistingArchived => {
                sentences.push("The existing Arrangement is archived.".to_owned());
            }
        }
    }
    sentences.join(" ")
}

/// The footer's summary of what approving would do.
pub fn summary(tally: &Tally) -> String {
    let plural = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    let writes = tally.new_songs + tally.new_arrangements + tally.updates;
    if writes == 0 && tally.open == 0 && tally.invalid == 0 {
        return "Everything is already in the Library.".to_owned();
    }
    let mut parts = Vec::new();
    if tally.new_songs > 0 {
        parts.push(plural(tally.new_songs, "new Song", "new Songs"));
    }
    if tally.new_arrangements > 0 {
        parts.push(plural(
            tally.new_arrangements,
            "new Arrangement",
            "new Arrangements",
        ));
    }
    if tally.updates > 0 {
        parts.push(plural(tally.updates, "update", "updates"));
    }
    if tally.new_choirs > 0 {
        parts.push(plural(tally.new_choirs, "new Choir", "new Choirs"));
    }
    if tally.skipped > 0 {
        parts.push(format!("{} skipped", tally.skipped));
    }
    if tally.invalid > 0 {
        parts.push(format!("{} invalid", tally.invalid));
    }
    if tally.open > 0 {
        parts.push(plural(tally.open, "decision to make", "decisions to make"));
    }
    parts.join(" \u{b7} ")
}

/// The first finding of an Invalid document in operator words.
pub fn invalid_sentence(reasons: &[InvalidReason], file: &str) -> String {
    let Some(first) = reasons.first() else {
        return format!("Couldn't read {file}.");
    };
    let (pointer, text) = first
        .message
        .split_once(" \u{b7} ")
        .unwrap_or(("", first.message.as_str()));
    match first.code.as_str() {
        "container" if first.message.contains("manifest") => {
            "Its contents don't match its manifest.".to_owned()
        }
        "container" => "It isn't an Ordo file.".to_owned(),
        "wrong-kind" => {
            let kind = first
                .message
                .strip_prefix("This is a ")
                .and_then(|rest| rest.split_once(" file, not"))
                .map(|(kind, _)| kind);
            kind.map_or_else(
                || "It isn't a Song import.".to_owned(),
                |kind| format!("It's a {kind} file, not a Song import."),
            )
        }
        "hash-mismatch" => "Its content hash doesn't match its lyrics.".to_owned(),
        "unsupported-version" => {
            "This file needs a newer version of Ordo. Update Ordo to import it.".to_owned()
        }
        "missing-reference" => {
            let id = text
                .strip_prefix("references unknown section ")
                .unwrap_or(text);
            format!("The Sequence refers to section {id}, which the document doesn't define.")
        }
        "schema" => format!("Its JSON doesn't follow the Song import format: {pointer} {text}."),
        "unreadable" => format!("Couldn't read {file}."),
        _ => {
            if text.ends_with('.') {
                text.to_owned()
            } else {
                format!("{text}.")
            }
        }
    }
}

/// One finding as the technical-details disclosure lists it.
pub fn technical_line(reason: &InvalidReason) -> String {
    match reason.message.split_once(" \u{b7} ") {
        Some((pointer, message)) => format!("{pointer} \u{b7} {} \u{b7} {message}", reason.code),
        None => format!("{} \u{b7} {}", reason.code, reason.message),
    }
}
