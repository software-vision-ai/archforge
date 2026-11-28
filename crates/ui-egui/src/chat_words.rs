//! Chat's words that are made rather than written: times as the history and the cards say them,
//! the provider badge, and a tool row's words from what the control channel answered.
//!
//! Times use the local UTC offset the host gives ([`crate::Clock`]); the window has no time zone of
//! its own, so without one a time is said in UTC and marked so. Months are short and translated.
//! A tool row is worded from its kept facts (the control method, the command, the elements it was
//! about and those its answer named), so a row read back from the project shows in the language of
//! the day, as it showed when it ran. The elements it names are counted as the notations handed
//! count them ([`NotationUi::chat_kind`], [`NotationUi::chat_count`]); the shell counts only its
//! own: diagrams, skills, and lines and elements.

use crate::ai_agents::Provider;
use crate::chat_store::{AnchorRole, ToolFacts, ToolStatus};
use crate::i18n::{fmt, tr, tr_ctx, trn};
use crate::notation_ui::{ChatKind, NotationUi};

/// The time now and the local offset from UTC, as the host gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// Seconds since 1970.
    pub now: u64,
    /// Seconds east of UTC; none when the host could not tell.
    pub offset: Option<i32>,
}

/// The days since 1970-01-01 as a civil date: year, month 1-12, day 1-31 (Howard Hinnant's days
/// from civil, inverted).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, u32::try_from(month).unwrap_or(1), u32::try_from(day).unwrap_or(1))
}

/// `secs` as local seconds: shifted by the offset, which is clamped to a real zone's range.
pub(crate) fn local(secs: u64, offset: Option<i32>) -> i64 {
    let secs = i64::try_from(secs).unwrap_or(i64::MAX);
    secs.saturating_add(i64::from(offset.unwrap_or(0).clamp(-14 * 3_600, 14 * 3_600)))
}

/// A time of day, `14:32`, local; with " UTC" after it when the offset is not known.
pub fn clock_time(secs: u64, offset: Option<i32>) -> String {
    let t = local(secs, offset).rem_euclid(86_400);
    let hhmm = format!("{:02}:{:02}", t / 3_600, (t % 3_600) / 60);
    match offset {
        Some(_) => hhmm,
        None => fmt(&tr("{time} UTC"), &[("time", &hhmm)]),
    }
}

/// A month's short name, translated.
pub fn month(m: u32) -> String {
    match m {
        1 => tr_ctx("month", "Jan"),
        2 => tr_ctx("month", "Feb"),
        3 => tr_ctx("month", "Mar"),
        4 => tr_ctx("month", "Apr"),
        5 => tr_ctx("month", "May"),
        6 => tr_ctx("month", "Jun"),
        7 => tr_ctx("month", "Jul"),
        8 => tr_ctx("month", "Aug"),
        9 => tr_ctx("month", "Sep"),
        10 => tr_ctx("month", "Oct"),
        11 => tr_ctx("month", "Nov"),
        _ => tr_ctx("month", "Dec"),
    }
}

/// When a conversation was last active, as the history says it: "Today 14:02", "Yesterday",
/// "3 Nov", and the year too when it is not this one.
pub fn when(then: u64, clock: Clock) -> String {
    let day = |s: u64| local(s, clock.offset).div_euclid(86_400);
    let (today, that) = (day(clock.now), day(then));
    if that == today {
        return fmt(&tr("Today {time}"), &[("time", &clock_time(then, clock.offset))]);
    }
    if that.saturating_add(1) == today {
        return tr("Yesterday");
    }
    date_on(that, today)
}

/// The day `that` as a date, in the language shown, with the year when it is not `today`'s.
fn date_on(that: i64, today: i64) -> String {
    let (y, m, d) = civil(that);
    let (this_year, _, _) = civil(today);
    if y == this_year {
        fmt(&tr("{day} {month}"), &[("day", &d.to_string()), ("month", &month(m))])
    } else {
        fmt(&tr("{day} {month} {year}"), &[("day", &d.to_string()), ("month", &month(m)), ("year", &y.to_string())])
    }
}

/// When a version was saved, as History says it: "8 Nov 15:30", local, with the year when it is
/// not this one.
pub fn date_and_time(then: u64, clock: Clock) -> String {
    let day = |s: u64| local(s, clock.offset).div_euclid(86_400);
    fmt(&tr("{date} {time}"), &[("date", &date_on(day(then), day(clock.now))), ("time", &clock_time(then, clock.offset))])
}

/// The seconds since 1970 of a time as [`rfc3339`] writes it, none for anything else: each
/// separator in its place, digits elsewhere, and a day the month has.
pub fn seconds_of(date: &str) -> Option<u64> {
    let canonical = date.len() == 20
        && date.bytes().enumerate().all(|(i, b)| match i {
            4 | 7 => b == b'-',
            10 => b == b'T',
            13 | 16 => b == b':',
            19 => b == b'Z',
            _ => b.is_ascii_digit(),
        });
    if !canonical {
        return None;
    }
    let digits = |from: usize, to: usize| date.get(from..to)?.parse::<i64>().ok();
    let (y, m, d) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let (hour, minute, second) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let last_day = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=12).contains(&m) || !(1..=last_day).contains(&d) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let of_era = y.rem_euclid(400);
    let of_year = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let days = era * 146_097 + of_era * 365 + of_era / 4 - of_era / 100 + of_year - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

/// A time as the project stores times: RFC 3339 in UTC to the second.
pub fn rfc3339(secs: u64) -> String {
    let s = i64::try_from(secs).unwrap_or(i64::MAX);
    let (y, m, d) = civil(s.div_euclid(86_400));
    let t = s.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", t / 3_600, (t % 3_600) / 60, t % 60)
}

/// The longest model name the badge shows; a longer one is cut.
const MODEL_CHARS: usize = 40;

/// A model as a person reads it: a Claude model the program named, `claude-sonnet-4-5-20250929`,
/// as "Sonnet 4.5"; any other name as given, without control characters, cut to 40 characters.
pub fn named_model(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let parts: Vec<&str> = lower.strip_prefix("claude-").unwrap_or(&lower).split('-').collect();
    let family = ["sonnet", "opus", "haiku"].into_iter().find(|f| parts.contains(f));
    let version: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit())).collect();
    if lower.starts_with("claude-")
        && let Some(f) = family
        && !version.is_empty()
    {
        let mut word = f.to_string();
        if let Some(first) = word.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        return format!("{word} {}", version.join("."));
    }
    name.chars().filter(|c| !c.is_control()).take(MODEL_CHARS).collect()
}

/// The provider badge's words: the agent, then its model, "Claude · Sonnet 4.5" once the program
/// named the model, "Claude · Sonnet" from the alias asked before; the program's default shows
/// the agent alone until the program names it.
pub fn badge(provider: Provider, model: &str, named: Option<&str>) -> String {
    let model = match named {
        Some(n) => named_model(n),
        None if model == "default" => return provider.name().to_string(),
        None => crate::ai_agents::model_name(model, provider),
    };
    fmt(&tr("{agent} · {model}"), &[("agent", provider.name()), ("model", &model)])
}

/// What a person counts an element as in a tool row's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind<'a> {
    /// A part of another element, not counted: a diagram's own nodes, lines, labels and styles,
    /// core's comment, and what a notation says is one.
    Part,
    /// A diagram.
    Diagram,
    /// A skill.
    Skill,
    /// A line: the element of a line, of any notation.
    Line,
    /// A notation's own kind: the notation's place among those handed, and its key.
    Own(usize, &'static str),
    /// Anything else, an element, told apart by its table.
    Element(&'a str),
}

/// The kind a person counts an element of the table `table` (a generated table's name) as: the
/// shell's own for a diagram, its parts, a skill and core's comment; else as the first of the
/// notations `uis` that knows the table says ([`NotationUi::chat_kind`]); else an element.
fn kind_of<'a>(uis: &[&'static dyn NotationUi], table: &'a str) -> Kind<'a> {
    match table {
        "diagram_diagram" => Kind::Diagram,
        "core_skill" => Kind::Skill,
        "core_comment" => Kind::Part,
        t if t.starts_with("diagram_") => Kind::Part,
        _ => uis
            .iter()
            .enumerate()
            .find_map(|(i, ui)| {
                Some(match ui.chat_kind(table)? {
                    ChatKind::Part => Kind::Part,
                    ChatKind::Line => Kind::Line,
                    ChatKind::Own(key) => Kind::Own(i, key),
                })
            })
            .unwrap_or(Kind::Element(table)),
    }
}

/// Whether an element of the table `table` counts as one a person reads about, as the notations
/// `uis` count it: a part of another element (a diagram's own nodes, lines, labels and styles,
/// core's comment, and what a notation says is one) is not one of its own.
pub(crate) fn counted(uis: &[&'static dyn NotationUi], table: &str) -> bool {
    kind_of(uis, table) != Kind::Part
}

/// A count of `n` elements of one kind, in the words a person uses: a notation's kind in its own
/// words ([`NotationUi::chat_count`]), "2 lines" for relationships, "3 elements" for the rest.
fn kinds(uis: &[&'static dyn NotationUi], kind: Kind<'_>, n: u64) -> String {
    let elements = || trn("{n} element", "{n} elements", n);
    match kind {
        Kind::Diagram => trn("{n} diagram", "{n} diagrams", n),
        Kind::Skill => trn("{n} skill", "{n} skills", n),
        Kind::Line => trn("{n} line", "{n} lines", n),
        Kind::Own(i, key) => uis.get(i).and_then(|ui| ui.chat_count(key, n)).unwrap_or_else(elements),
        Kind::Part | Kind::Element(_) => elements(),
    }
}

/// The elements a row's answer named, summed by kind as the notations `uis` count them, the two
/// most first; a part of another element is not counted. When the answer named more than are
/// kept, the count in all.
fn read_summary(uis: &[&'static dyn NotationUi], facts: &ToolFacts) -> Option<String> {
    let mut by: Vec<(Kind<'_>, u64)> = Vec::new();
    let mut kept = 0u64;
    for a in facts.anchors.iter().filter(|a| a.role == AnchorRole::Read) {
        let kind = kind_of(uis, &a.metaclass);
        if kind == Kind::Part {
            continue;
        }
        kept += 1;
        match by.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => by.push((kind, 1)),
        }
    }
    let all = facts.element_count.map_or(kept, u64::from);
    if all == 0 {
        return None;
    }
    if all > kept {
        return Some(kinds(uis, Kind::Element(""), all));
    }
    // Most first; among equals, the one the answer named first.
    by.sort_by_key(|k| std::cmp::Reverse(k.1));
    let words: Vec<String> = by.iter().take(2).map(|(k, n)| kinds(uis, *k, *n)).collect();
    match words.as_slice() {
        [one] => Some(one.clone()),
        [a, b] => Some(fmt(&tr("{first}, {second}"), &[("first", a), ("second", b)])),
        _ => None,
    }
}

/// What a row's words say it did, from the command it ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Read,
    Changed,
    Added,
    Removed,
}

/// The verb of a command: a call that ran and changed nothing read; one that changed the model
/// added (a command that adds, makes or places), removed (one that removes or deletes) or changed.
/// A call refused or failed is worded by what it asked: by its command's name, a read when the name
/// says it reads.
pub fn verb(command: Option<&str>, changed_model: bool, status: ToolStatus) -> Verb {
    if status == ToolStatus::Done && !changed_model {
        return Verb::Read;
    }
    let last = command.and_then(|c| c.rsplit('.').next()).unwrap_or_default();
    match last {
        "add" | "create" | "new" | "place" | "insert" | "make" | "duplicate" => Verb::Added,
        "remove" | "delete" | "destroy" => Verb::Removed,
        "get" | "list" | "find" | "inspect" | "read" | "search" | "count" | "query" | "show" if status != ToolStatus::Done => Verb::Read,
        _ if command.is_none() && status != ToolStatus::Done => Verb::Read,
        _ => Verb::Changed,
    }
}

/// A row's words: its label ("Read diagram Commerce", "Changed Customer", "Read 8 elements") and
/// its detail ("2 lines", "description", "refused"), the elements counted as the notations `uis`
/// count them.
#[cfg(any())] // AR-796 parked
pub fn tool_words(uis: &[&'static dyn NotationUi], facts: &ToolFacts) -> (String, Option<String>) {
    let detail_of = |d: Option<String>| match facts.status {
        ToolStatus::Done => d,
        ToolStatus::Refused => Some(tr("refused")),
        ToolStatus::Failed => Some(tr("failed")),
    };
    match facts.tool.as_str() {
        "engine.undo" => return (tr("Undid a step"), detail_of(None)),
        "engine.redo" => return (tr("Redid a step"), detail_of(None)),
        "app.save" | "app.saved" => return (tr("Saved the project"), detail_of(None)),
        "document.inspect" => return (tr("Read the project"), detail_of(None)),
        _ => {}
    }
    let v = verb(facts.command.as_deref(), facts.changed_model, facts.status);
    let targets: Vec<&crate::chat_store::Anchor> = facts.anchors.iter().filter(|a| a.role == AnchorRole::Target).collect();
    let with = |what: &str| -> String {
        let template = match v {
            Verb::Read => tr("Read {what}"),
            Verb::Changed => tr("Changed {what}"),
            Verb::Added => tr("Added {what}"),
            Verb::Removed => tr("Removed {what}"),
        };
        fmt(&template, &[("what", what)])
    };
    let label = match targets.as_slice() {
        [one] => {
            let name = one.name.clone().filter(|n| !n.is_empty());
            match (name, v) {
                (Some(n), Verb::Read) if one.metaclass == "diagram_diagram" => fmt(&tr("Read diagram {name}"), &[("name", &n)]),
                (Some(n), _) => with(&n),
                (None, _) => with(&kinds(uis, kind_of(uis, &one.metaclass), 1)),
            }
        }
        [] if v == Verb::Read => match read_summary(uis, facts) {
            Some(what) => {
                // Read with no element it was about: what its answer named is the label, and the
                // aspect read the detail ("Read 8 elements · descriptions").
                return (with(&what), detail_of(facts.detail.clone()));
            }
            None => tr("Read the model"),
        },
        [] => tr("Changed the model"),
        many => {
            let first = many.first().map(|a| a.metaclass.as_str()).unwrap_or_default();
            let same = many.iter().all(|a| a.metaclass == first);
            with(&kinds(uis, kind_of(uis, if same { first } else { "" }), u64::try_from(many.len()).unwrap_or(u64::MAX)))
        }
    };
    let detail = match v {
        Verb::Read => read_summary(uis, facts).or_else(|| facts.detail.clone()),
        Verb::Changed => facts.detail.clone(),
        // What was added or removed is named in the label; the parameters say nothing more.
        Verb::Added | Verb::Removed => None,
    };
    (label, detail_of(detail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_store::Anchor;
    use crate::tests::SCREENS;
    use archforge_mof::Uuid;

    /// An anchor of the test's own.
    fn at(n: u8, role: AnchorRole, metaclass: &str, name: &str) -> Anchor {
        Anchor { element: Uuid([n; 16]), role, metaclass: metaclass.into(), name: Some(name.into()) }
    }

    /// Times are local, days named, months short and translated, UTC said when the offset is unknown.
    #[test]
    fn dates_and_times_are_local_with_short_translated_months() {
        crate::i18n::set_language("en");
        // 2026-11-07 14:02:00 UTC.
        let now = 1_794_060_120;
        let clock = Clock { now, offset: Some(3_600) };
        assert_eq!(rfc3339(now), "2026-11-07T14:02:00Z");
        assert_eq!(seconds_of("2026-11-07T14:02:00Z"), Some(now), "read back as it was written");
        for not_a_time in [
            "2026-13-07T14:02:00Z",
            "2026-00-07T14:02:00Z",
            "2026-11-32T14:02:00Z",
            "2026-11-07T24:02:00Z",
            "2026-11-07 14:02",
            "yesterday",
            "2026/11/07 14:02:00Z",
            "2026-11-07T14:02:00X",
            "+026-11-07T14:02:00Z",
            "2026-02-31T10:00:00Z",
            "2026-04-31T10:00:00Z",
            "2027-02-29T10:00:00Z",
            "2100-02-29T10:00:00Z",
        ] {
            assert_eq!(seconds_of(not_a_time), None, "{not_a_time}");
        }
        assert_eq!(seconds_of("2028-02-29T10:00:00Z"), Some(1_835_431_200), "a leap year has 29 February");
        assert_eq!(seconds_of("2000-02-29T00:00:00Z"), Some(951_782_400), "and so does a fourth century");
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(when(now, clock), "Today 15:02", "the local offset moves the clock");
        assert_eq!(when(now - 86_400, clock), "Yesterday");
        assert_eq!(when(now - 4 * 86_400, clock), "3 Nov");
        assert_eq!(when(now - 400 * 86_400, clock), "3 Oct 2025");
        // Just before local midnight a UTC time of the same day is already tomorrow's.
        let late = Clock { now: now + 9 * 3_600, offset: Some(3_600) };
        assert_eq!(when(now + 9 * 3_600, late), "Today 00:02");
        assert_eq!(when(now, late), "Yesterday");
        assert_eq!(clock_time(now, None), "14:02 UTC", "without an offset the time says it is UTC");
        crate::i18n::set_language("de");
        assert_eq!(when(now - 4 * 86_400, clock), "3. Nov.");
        crate::i18n::set_language("en");
    }

    /// The badge names the alias asked for, then the model the program named.
    #[test]
    fn the_badge_names_the_alias_until_the_program_names_the_model() {
        crate::i18n::set_language("en");
        assert_eq!(badge(Provider::Claude, "sonnet", None), "Claude · Sonnet");
        assert_eq!(badge(Provider::Claude, "sonnet", Some("claude-sonnet-4-5-20250929")), "Claude · Sonnet 4.5");
        assert_eq!(badge(Provider::Claude, "default", None), "Claude");
        assert_eq!(badge(Provider::Claude, "default", Some("claude-opus-4-1-20250805")), "Claude · Opus 4.1");
        assert_eq!(badge(Provider::Codex, "default", Some("gpt-5-codex")), "Codex · gpt-5-codex");
        assert_eq!(named_model("claude-3-5-haiku-20241022"), "Haiku 3.5");
        assert_eq!(named_model(&format!("x\u{7}{}", "y".repeat(80))).chars().count(), MODEL_CHARS);
    }

    /// A tool row's words come from its kept facts: verb, subject, what was read, refused or failed.
    #[test]
    fn a_row_is_worded_from_what_the_channel_answered() {
        let uis = SCREENS.notation_uis();
        crate::i18n::set_language("en");
        let facts = |tool: &str, command: Option<&str>, changed: bool, anchors: Vec<Anchor>| ToolFacts {
            tool: tool.into(),
            command: command.map(str::to_string),
            detail: None,
            status: ToolStatus::Done,
            changed_model: changed,
            element_count: None,
            anchors,
        };
        let classes: Vec<Anchor> = (1..=8).map(|n| at(n, AnchorRole::Read, "uml_class", "C")).collect();
        let mut diagram = classes.clone();
        diagram.push(at(20, AnchorRole::Target, "diagram_diagram", "Commerce"));
        diagram.push(at(21, AnchorRole::Read, "diagram_node", ""));
        assert_eq!(
            tool_words(uis, &facts("engine.execute", Some("diagram.get"), false, diagram)),
            ("Read diagram Commerce".into(), Some("8 classes".into()))
        );
        let mut listed = facts("engine.execute", Some("sketch.class.list"), false, classes);
        listed.detail = Some("descriptions".into());
        assert_eq!(tool_words(uis, &listed), ("Read 8 classes".into(), Some("descriptions".into())));
        let payment = vec![
            at(1, AnchorRole::Target, "uml_class", "Payment"),
            at(2, AnchorRole::Read, "uml_property", "amount"),
            at(3, AnchorRole::Read, "uml_property", "id"),
            at(4, AnchorRole::Read, "uml_association", ""),
            at(5, AnchorRole::Read, "uml_generalization", ""),
        ];
        assert_eq!(tool_words(uis, &facts("engine.execute", Some("uml.class.get"), false, payment)).1.as_deref(), Some("2 attributes, 2 lines"));
        let mut changed = facts("engine.execute", Some("uml.class.set"), true, vec![at(1, AnchorRole::Target, "uml_class", "Customer")]);
        changed.detail = Some("description".into());
        assert_eq!(tool_words(uis, &changed), ("Changed Customer".into(), Some("description".into())));
        let added = facts("engine.execute", Some("sketch.class.add"), true, vec![at(1, AnchorRole::Target, "uml_class", "Invoice")]);
        assert_eq!(tool_words(uis, &added).0, "Added Invoice");
        let removed = facts("engine.execute", Some("er.entity.remove"), true, vec![at(1, AnchorRole::Target, "er_entity", "Tag")]);
        assert_eq!(tool_words(uis, &removed).0, "Removed Tag");
        let mut refused = removed.clone();
        refused.status = ToolStatus::Refused;
        assert_eq!(tool_words(uis, &refused), ("Removed Tag".into(), Some("refused".into())));
        let mut failed = added;
        failed.status = ToolStatus::Failed;
        assert_eq!(tool_words(uis, &failed).1.as_deref(), Some("failed"));
        assert_eq!(tool_words(uis, &facts("engine.undo", None, true, Vec::new())).0, "Undid a step");
        // More named than kept: the count in all.
        let mut many = facts("engine.execute", Some("sketch.class.list"), false, vec![at(1, AnchorRole::Read, "uml_class", "A")]);
        many.element_count = Some(700);
        assert_eq!(tool_words(uis, &many).0, "Read 700 elements");
    }

    /// The app's notations count each of their kinds in its own words, every relationship of
    /// either as a line, a primitive type as a data type, and leave out the parts of a diagram and
    /// a description's comment; the shell counts diagrams, skills and the rest as elements.
    #[test]
    fn the_app_notations_count_their_kinds_in_their_words() {
        let uis = SCREENS.notation_uis();
        crate::i18n::set_language("en");
        let read = |metaclasses: &[&str]| ToolFacts {
            tool: "engine.execute".into(),
            command: Some("model.find".into()),
            detail: None,
            status: ToolStatus::Done,
            changed_model: false,
            element_count: None,
            anchors: metaclasses.iter().zip(1u8..).map(|(m, n)| at(n, AnchorRole::Read, m, "")).collect(),
        };
        for (table, two) in [
            ("uml_class", "2 classes"),
            ("uml_interface", "2 interfaces"),
            ("uml_enumeration", "2 enumerations"),
            ("uml_data_type", "2 data types"),
            ("uml_primitive_type", "2 data types"),
            ("uml_package", "2 packages"),
            ("uml_property", "2 attributes"),
            ("uml_operation", "2 operations"),
            ("uml_enumeration_literal", "2 literals"),
            ("er_entity", "2 tables"),
            ("er_attribute", "2 columns"),
            ("er_schema", "2 schemas"),
            ("diagram_diagram", "2 diagrams"),
            ("core_skill", "2 skills"),
            ("uml_constraint", "2 elements"),
        ] {
            assert_eq!(tool_words(uis, &read(&[table, table])).0, format!("Read {two}"), "{table}");
        }
        let lines = [
            "uml_association",
            "uml_association_class",
            "uml_generalization",
            "uml_dependency",
            "uml_realization",
            "uml_abstraction",
            "uml_usage",
            "uml_interface_realization",
            "er_relationship",
            "er_generalization",
        ];
        assert_eq!(tool_words(uis, &read(&lines)).0, "Read 10 lines", "every relationship of either notation");
        assert_eq!(tool_words(uis, &read(&["uml_data_type", "uml_primitive_type", "er_entity"])).0, "Read 2 data types, 1 table");
        let parts = ["diagram_node", "diagram_edge", "core_comment", "uml_comment", "er_entity"];
        assert_eq!(tool_words(uis, &read(&parts)).0, "Read 1 table", "a diagram's parts and a description's comment are not counted");
        // Elements of tables no one counts are told apart by their table.
        assert_eq!(tool_words(uis, &read(&["core_folder", "core_folder", "core_model"])).0, "Read 2 elements, 1 element");
    }
}
