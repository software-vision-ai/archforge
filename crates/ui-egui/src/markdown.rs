//! Markdown as a skill's text is written in: the styled source the editor draws (the Markdown
//! itself, its markers dimmed, headings larger, strong text semibold, emphasis slanted, code in
//! the monospace face, quotes and links in their colours) and the toolbar's edits, which wrap the
//! selection or mark its lines. The text is never rewritten by the styling: what is typed is what
//! is kept. pulldown-cmark reads it, giving every element's place in the text.
//!
//! A text read from a project may be far longer than one typed here, or nested a level per
//! character; styling and the rendered page walk every level, so a text over the budget
//! (`within_budget`) is shown plain instead.

use std::ops::Range;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};

use crate::ux::{Palette, types};

/// How one byte of the source is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Look {
    heading: u8,
    strong: bool,
    italic: bool,
    code: bool,
    dim: bool,
    quote: bool,
    link: bool,
}

/// The deepest nesting styled or rendered: quotes, lists and spans inside one another.
pub const MAX_DEPTH: usize = 32;

/// The extensions a skill's Markdown may use besides CommonMark.
fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES
}

/// The extensions the rendered page reads with (the viewer's own set): footnotes and definition
/// lists are containers too, so the depth is counted as the viewer will meet it.
fn viewer_options() -> Options {
    options() | Options::ENABLE_FOOTNOTES | Options::ENABLE_DEFINITION_LIST
}

/// Whether `text` is short enough and shallow enough to style and to render: no longer than a
/// skill's text may be, and nested no deeper than `MAX_DEPTH` as the styling or the rendered page
/// reads it.
pub fn within_budget(text: &str) -> bool {
    if text.chars().count() > archforge_core::skills::MAX_BODY {
        return false;
    }
    let mut depth = 0usize;
    for event in Parser::new_ext(text, viewer_options()) {
        match event {
            Event::Start(_) => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return false;
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

/// How many bytes of `range` from its start satisfy `f`; none when the range is not in `bytes`.
fn lead(bytes: &[u8], range: Range<usize>, f: impl Fn(u8) -> bool) -> usize {
    bytes.get(range).map_or(0, |b| b.iter().take_while(|b| f(**b)).count())
}

/// Every byte's look, from the parser's elements and their places.
fn looks(text: &str) -> Vec<Look> {
    let mut out = vec![Look::default(); text.len()];
    let bytes = text.as_bytes();
    let mut set = |range: Range<usize>, f: &dyn Fn(&mut Look)| {
        for look in out.get_mut(range.start.min(text.len())..range.end.min(text.len())).into_iter().flatten() {
            f(look);
        }
    };
    // The marker bytes at an element's ends: `n` from its start and `m` before its end.
    let ends = |range: &Range<usize>, n: usize, m: usize| {
        [range.start..(range.start + n).min(range.end), range.end.saturating_sub(m).max(range.start)..range.end]
    };
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let n = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    _ => 4,
                };
                set(range.clone(), &|l| l.heading = n);
                // The leading hashes and the space after them.
                let hashes = lead(bytes, range.clone(), |b| b == b'#');
                let spaces = lead(bytes, range.start + hashes..range.end, |b| b == b' ');
                set(range.start..range.start + hashes + spaces, &|l| l.dim = true);
            }
            Event::Start(Tag::Strong) => {
                set(range.clone(), &|l| l.strong = true);
                for r in ends(&range, 2, 2) {
                    set(r, &|l| l.dim = true);
                }
            }
            Event::Start(Tag::Emphasis) => {
                set(range.clone(), &|l| l.italic = true);
                for r in ends(&range, 1, 1) {
                    set(r, &|l| l.dim = true);
                }
            }
            Event::Start(Tag::Strikethrough) => {
                for r in ends(&range, 2, 2) {
                    set(r, &|l| l.dim = true);
                }
            }
            Event::Code(_) => {
                set(range.clone(), &|l| l.code = true);
                let ticks = lead(bytes, range.clone(), |b| b == b'`');
                for r in ends(&range, ticks, ticks) {
                    set(r, &|l| l.dim = true);
                }
            }
            Event::Start(Tag::CodeBlock(_)) => {
                set(range.clone(), &|l| l.code = true);
                // A fenced block's fence lines.
                for line in line_ranges(text, range.clone()) {
                    let trimmed = text.get(line.clone()).unwrap_or_default().trim_start();
                    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                        set(line, &|l| l.dim = true);
                    }
                }
            }
            Event::Start(Tag::Item) => {
                let at = range.start + lead(bytes, range.clone(), |b| b == b' ');
                let rest = bytes.get(at..range.end).unwrap_or_default();
                let marker = match rest.first() {
                    Some(b'-' | b'*' | b'+') => 1,
                    Some(b) if b.is_ascii_digit() => {
                        let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
                        if matches!(rest.get(digits), Some(b'.' | b')')) { digits + 1 } else { 0 }
                    }
                    _ => 0,
                };
                set(at..(at + marker + 1).min(range.end), &|l| l.dim = true);
            }
            Event::TaskListMarker(_) => set(range, &|l| l.dim = true),
            Event::Start(Tag::BlockQuote(_)) => {
                set(range.clone(), &|l| l.quote = true);
                for line in line_ranges(text, range) {
                    let n = lead(bytes, line.clone(), |b| b == b' ' || b == b'>');
                    set(line.start..line.start + n, &|l| l.dim = true);
                }
            }
            Event::Start(Tag::Link { .. }) => {
                set(range.clone(), &|l| l.link = true);
                // `[label](target)`: the brackets and the target are the link's markup.
                if bytes.get(range.start) == Some(&b'[') {
                    set(range.start..range.start + 1, &|l| l.dim = true);
                    if let Some(close) = text.get(range.clone()).and_then(|t| t.find("](")) {
                        set(range.start + close..range.end, &|l| l.dim = true);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The lines of `text` within `range`, without their line breaks.
fn line_ranges(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = range.start;
    for (i, b) in text.as_bytes().get(range.clone()).unwrap_or_default().iter().enumerate() {
        if *b == b'\n' {
            out.push(start..range.start + i);
            start = range.start + i + 1;
        }
    }
    if start < range.end {
        out.push(start..range.end);
    }
    out
}

/// The format one look is drawn in.
fn format(look: Look, p: &Palette) -> TextFormat {
    let (size, line, semibold) = match look.heading {
        1 => (22.0, 32.0, true),
        2 => (18.0, 27.0, true),
        3 => (16.0, 24.0, true),
        4 => (types::BODY.size, 22.0, true),
        _ => (types::BODY.size, 22.0, look.strong),
    };
    let font = if look.code {
        FontId::new(size - 1.0, FontFamily::Monospace)
    } else if semibold {
        FontId::new(size, types::BODY_STRONG.font().family)
    } else {
        FontId::new(size, types::BODY.font().family)
    };
    let color: Color32 = if look.dim {
        p.text_muted
    } else if look.code {
        p.purple
    } else if look.link {
        p.accent
    } else if look.quote {
        p.text_secondary
    } else {
        p.text
    };
    TextFormat { font_id: font, color, italics: look.italic && !look.code, line_height: Some(line), ..TextFormat::default() }
}

/// The styled source of `text`, wrapped at `wrap` points; plain when it is over the budget.
pub fn layout_job(text: &str, p: &Palette, wrap: f32) -> LayoutJob {
    let looks = if within_budget(text) { looks(text) } else { vec![Look::default(); text.len()] };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut start = 0;
    while start < text.len() {
        let look = looks[start];
        let mut end = start + 1;
        while end < text.len() && (looks[end] == look || !text.is_char_boundary(end)) {
            end += 1;
        }
        job.append(&text[start..end], 0.0, format(look, p));
        start = end;
    }
    if text.is_empty() {
        job.append("", 0.0, format(Look::default(), p));
    }
    job
}

/// What a toolbar button does to the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bold,
    Italic,
    Heading,
    Bulleted,
    Numbered,
    Code,
    Link,
    Quote,
}

impl Format {
    /// Every button, in the toolbar's order, with its automation key and hover text.
    pub const ALL: [(Format, &'static str, &'static str); 8] = [
        (Format::Bold, "bold", "Bold"),
        (Format::Italic, "italic", "Italic"),
        (Format::Heading, "heading", "Heading"),
        (Format::Bulleted, "bulleted", "Bulleted list"),
        (Format::Numbered, "numbered", "Numbered list"),
        (Format::Code, "code", "Code"),
        (Format::Link, "link", "Link"),
        (Format::Quote, "quote", "Quote"),
    ];
}

/// The byte offset of char index `c` in `text`.
fn byte_at(text: &str, c: usize) -> usize {
    text.char_indices().nth(c).map_or(text.len(), |(b, _)| b)
}

/// The char index of byte offset `b` in `text`.
fn char_at(text: &str, b: usize) -> usize {
    text[..b.min(text.len())].chars().count()
}

/// Applies `f` to `text` around the selection `sel` (char indices, start not after end), and gives
/// the selection after it, in char indices: the wrapped words stay selected, a link's target is
/// selected for typing, marked lines keep their place.
pub fn apply(text: &mut String, sel: Range<usize>, f: Format) -> Range<usize> {
    // A selection given end first, or past the end, is taken in order and within the text.
    let len = text.chars().count();
    let sel = sel.start.min(sel.end).min(len)..sel.start.max(sel.end).min(len);
    let (a, b) = (byte_at(text, sel.start), byte_at(text, sel.end));
    let wrapped = |text: &mut String, mark: &str| -> Range<usize> {
        let before = &text[..a];
        let after = &text[b..];
        if before.ends_with(mark) && after.starts_with(mark) && !(mark == "*" && before.ends_with("**") && after.starts_with("**")) {
            // Marked already: the marks go.
            text.replace_range(b..b + mark.len(), "");
            text.replace_range(a - mark.len()..a, "");
            let start = char_at(text, a - mark.len());
            return start..start + (sel.end - sel.start);
        }
        text.insert_str(b, mark);
        text.insert_str(a, mark);
        let start = sel.start + mark.chars().count();
        start..start + (sel.end - sel.start)
    };
    match f {
        Format::Bold => wrapped(text, "**"),
        Format::Italic => wrapped(text, "*"),
        Format::Code => wrapped(text, "`"),
        Format::Link => {
            let label = text[a..b].to_string();
            let label = if label.is_empty() { "text".to_string() } else { label };
            let link = format!("[{label}](url)");
            text.replace_range(a..b, &link);
            if sel.start == sel.end {
                sel.start + 1..sel.start + 1 + label.chars().count()
            } else {
                let target = sel.start + 1 + label.chars().count() + 2;
                target..target + 3
            }
        }
        Format::Heading | Format::Bulleted | Format::Numbered | Format::Quote => lines(text, a, b, sel, f),
    }
}

/// A count as a signed number; no text is long enough to reach the limit.
fn signed(n: usize) -> isize {
    isize::try_from(n).unwrap_or(isize::MAX)
}

/// Marks, or unmarks, every line the selection touches.
fn lines(text: &mut String, a: usize, b: usize, sel: Range<usize>, f: Format) -> Range<usize> {
    let first = text[..a].rfind('\n').map_or(0, |i| i + 1);
    let last = text[b..].find('\n').map_or(text.len(), |i| b + i);
    let block: Vec<String> = text[first..last].split('\n').map(str::to_string).collect();
    let marked = |line: &str| -> Option<usize> {
        match f {
            Format::Heading => {
                let n = line.chars().take_while(|c| *c == '#').count();
                (n > 0 && line[n..].starts_with(' ')).then_some(n + 1)
            }
            Format::Bulleted => (line.starts_with("- ") || line.starts_with("* ")).then_some(2),
            Format::Numbered => {
                let n = line.chars().take_while(char::is_ascii_digit).count();
                (n > 0 && line[n..].starts_with(". ")).then_some(n + 2)
            }
            Format::Quote => line.starts_with("> ").then_some(2),
            _ => None,
        }
    };
    let all = block.iter().all(|l| marked(l).is_some());
    let mut out = Vec::with_capacity(block.len());
    let mut moved: isize = 0;
    let mut moved_first: isize = 0;
    for (i, line) in block.iter().enumerate() {
        let next = if f == Format::Heading {
            // A heading goes one level deeper on each press, up to three, then back to text.
            match marked(line) {
                Some(n) if n - 1 < 3 => format!("#{line}"),
                Some(n) => line[n..].to_string(),
                None => format!("# {line}"),
            }
        } else if all {
            line[marked(line).unwrap_or(0)..].to_string()
        } else {
            let bare = &line[marked(line).unwrap_or(0)..];
            match f {
                Format::Bulleted => format!("- {bare}"),
                Format::Numbered => format!("{}. {bare}", i + 1),
                _ => format!("> {bare}"),
            }
        };
        let delta = signed(next.chars().count()) - signed(line.chars().count());
        if i == 0 {
            moved_first = delta;
        }
        moved += delta;
        out.push(next);
    }
    text.replace_range(first..last, &out.join("\n"));
    let shift = |c: usize, by: isize| usize::try_from((signed(c) + by).max(signed(char_at(text, first)))).unwrap_or(0);
    shift(sel.start, moved_first)..shift(sel.end, moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim_text(text: &str) -> String {
        let looks = looks(text);
        text.char_indices().filter(|(i, _)| looks[*i].dim).map(|(_, c)| c).collect()
    }

    #[test]
    fn markers_are_dimmed_and_the_words_keep_their_look() {
        let text = "# Naming\n\nUse **snake_case** and `ids`, see [the guide](https://x).\n\n- one\n1. two\n> quoted";
        assert_eq!(dim_text(text), "# ****``[](https://x)- 1. > ");
        let looks = looks(text);
        let at = |s: &str| text.find(s).unwrap();
        assert_eq!(looks[at("Naming")].heading, 1);
        assert!(looks[at("snake_case")].strong && !looks[at("snake_case")].dim);
        assert!(looks[at("ids")].code);
        assert!(looks[at("the guide")].link);
        assert!(looks[at("quoted")].quote);
    }

    #[test]
    fn the_styled_source_keeps_the_text_as_typed() {
        let p = crate::ux::palette(false);
        for text in ["", "plain", "# Ä heading with **bold** é", "```\ncode\n```\n"] {
            assert_eq!(layout_job(text, p, 400.0).text, text);
        }
    }

    #[test]
    fn the_toolbar_wraps_and_unwraps_the_selection() {
        let mut text = "make it bold".to_string();
        let sel = apply(&mut text, 8..12, Format::Bold);
        assert_eq!((text.as_str(), sel.clone()), ("make it **bold**", 10..14));
        let sel = apply(&mut text, sel, Format::Bold);
        assert_eq!((text.as_str(), sel), ("make it bold", 8..12));
        let mut text = "x".to_string();
        let sel = apply(&mut text, 1..1, Format::Code);
        assert_eq!((text.as_str(), sel), ("x``", 2..2));
        let mut text = "see docs".to_string();
        let sel = apply(&mut text, 4..8, Format::Link);
        assert_eq!((text.as_str(), sel), ("see [docs](url)", 11..14));
    }

    #[test]
    fn a_text_over_the_budget_is_shown_plain_and_awkward_text_never_panics() {
        let p = crate::ux::palette(false);
        let deep = ">".repeat(4000);
        assert!(!within_budget(&deep));
        // Containers the rendered page reads besides CommonMark's.
        assert!(!within_budget(&format!("[^a]: {}", ">".repeat(3990))));
        assert!(!within_budget(&format!("t\n: {}", ">".repeat(3990))));
        assert!(!within_budget(&"a".repeat(archforge_core::skills::MAX_BODY + 1)));
        assert!(within_budget(&"> ".repeat(MAX_DEPTH - 1)));
        let started = std::time::Instant::now();
        assert_eq!(layout_job(&">".repeat(65_535), p, 400.0).text.len(), 65_535);
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
        for text in [
            "é# ü\n#é",
            "**ä**é*ö*",
            "`é`ü``",
            "```é\nü",
            "~~~\r\nx\r\n~~~",
            "- é\n\t1) ü",
            "> é\n>> ü\n>",
            "[é](ü)[",
            "###",
            "``",
            "|a|b|\n|-|-|\n|é|ü|",
            "- [x] é",
            "\r\n\r\n",
            "*",
            "#",
        ] {
            assert_eq!(layout_job(text, p, 400.0).text, text);
            for f in Format::ALL.map(|(f, _, _)| f) {
                for sel in [0..0, 0..text.chars().count(), Range { start: 3, end: 1 }, 1..999] {
                    let mut t = text.to_string();
                    let got = apply(&mut t, sel, f);
                    assert!(got.start <= got.end && got.end <= t.chars().count(), "{text:?} {f:?} {got:?}");
                }
            }
        }
    }

    #[test]
    fn the_toolbar_marks_and_unmarks_every_line_touched() {
        let mut text = "one\ntwo\nthree".to_string();
        let sel = apply(&mut text, 1..5, Format::Numbered);
        assert_eq!(text, "1. one\n2. two\nthree");
        assert_eq!(sel, 4..11);
        apply(&mut text, 4..11, Format::Numbered);
        assert_eq!(text, "one\ntwo\nthree");
        let mut text = "Title".to_string();
        for want in ["# Title", "## Title", "### Title", "Title"] {
            apply(&mut text, 0..0, Format::Heading);
            assert_eq!(text, want);
        }
        let mut text = "ä quote".to_string();
        apply(&mut text, 0..0, Format::Quote);
        assert_eq!(text, "> ä quote");
    }
}
