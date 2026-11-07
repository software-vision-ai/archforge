//! An agent's reply as Chat shows it: a small Markdown renderer at the design's reply metrics
//! (13 on 19). Paragraphs, headings (as strong text: the panel is narrow), strong and slanted text,
//! code, lists, quotes and links. An image shows its alt text and HTML shows as the plain text it
//! is, so a reply can load nothing and run nothing. A link opens only through the confirmation, and
//! only when it is a web address that shows what it is ([`web_link`]); any other link is shown as
//! its words alone.
//!
//! A reply is the agent's text, so it is untrusted: it is read with a budget of events and a depth
//! cap, and one past either is shown as plain text.

use std::ops::Range;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, Stroke};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::ux::{Palette, types};

/// The deepest nesting rendered: quotes, lists and spans inside one another.
const MAX_DEPTH: usize = 32;
/// The most Markdown events rendered; a reply with more is shown plain.
const MAX_EVENTS: usize = 200_000;

/// A rendered reply: its layout, and the links in it as byte ranges of the layout's text with the
/// address each opens.
pub struct Rendered {
    pub job: LayoutJob,
    pub links: Vec<(Range<usize>, String)>,
}

/// The address a link opens, or none: a web address (http or https) with a host, no user part,
/// and nothing but visible ASCII in it: no hidden character, no space, and no letter from another
/// script that could make a host look like another (an international domain comes as `xn--`).
pub fn web_link(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url.len() > 2_048 || !url.chars().all(|c| c.is_ascii_graphic()) || url.contains('\\') {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once(':').map_or(authority, |(h, port)| if port.bytes().all(|b| b.is_ascii_digit()) { h } else { authority });
    let plain =
        !host.is_empty() && !authority.contains('@') && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'));
    plain.then(|| url.to_string())
}

/// How one run of text is drawn.
#[derive(Debug, Clone, Copy, Default)]
struct Look {
    strong: bool,
    italic: bool,
    code: bool,
    quote: bool,
    link: bool,
    muted: bool,
}

/// The text format a run of `look` is drawn in, at the reply's 13 on 19.
fn format(look: Look, p: &Palette) -> TextFormat {
    let font = if look.code {
        FontId::new(12.0, FontFamily::Monospace)
    } else if look.strong {
        types::UI_STRONG.font()
    } else {
        types::UI.font()
    };
    let color: Color32 = if look.link {
        p.accent
    } else if look.quote || look.muted {
        p.text_secondary
    } else {
        p.text
    };
    TextFormat {
        font_id: font,
        color,
        italics: look.italic && !look.code,
        background: if look.code { p.field } else { Color32::TRANSPARENT },
        underline: if look.link { Stroke::new(1.0, p.accent) } else { Stroke::NONE },
        line_height: Some(types::UI.line),
        ..TextFormat::default()
    }
}

/// A small gap between blocks: an empty line of a third of the reply's line.
fn gap(job: &mut LayoutJob, p: &Palette) {
    job.append(
        "\n",
        0.0,
        TextFormat { font_id: FontId::new(6.0, types::UI.font().family), line_height: Some(7.0), color: p.text, ..TextFormat::default() },
    );
}

/// `text` as plain words at the reply's metrics.
pub fn plain(text: &str, p: &Palette, wrap: f32) -> Rendered {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap.max(1.0);
    job.append(text, 0.0, format(Look::default(), p));
    Rendered { job, links: Vec::new() }
}

/// Whether `text` is shallow and short enough to render as Markdown.
fn within_budget(text: &str) -> bool {
    let mut depth = 0usize;
    for (n, event) in Parser::new_ext(text, options()).enumerate() {
        if n >= MAX_EVENTS {
            return false;
        }
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

/// The Markdown extensions a reply is read with.
fn options() -> Options {
    Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES
}

/// `text` rendered, wrapped at `wrap` points; plain past the budget.
pub fn render(text: &str, p: &Palette, wrap: f32) -> Rendered {
    if !within_budget(text) {
        return plain(text, p, wrap);
    }
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap.max(1.0);
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    let mut look = Look::default();
    // The lists open, each with its next number when numbered; the link being written and where
    // its words started; whether the next text starts a new line of its block.
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut link: Option<(usize, Option<String>)> = None;
    let mut image = 0usize;
    let mut quotes = 0usize;
    let mut started = false;
    let mut line_start = true;
    let push = |job: &mut LayoutJob, s: &str, look: Look, line_start: &mut bool, quotes: usize| {
        if s.is_empty() {
            return;
        }
        if *line_start && quotes > 0 {
            job.append(&"│ ".repeat(quotes), 0.0, format(Look { quote: true, ..Look::default() }, p));
        }
        *line_start = s.ends_with('\n');
        job.append(s, 0.0, format(look, p));
    };
    for event in Parser::new_ext(text, options()) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph | Tag::Heading { .. } | Tag::CodeBlock(_) | Tag::Table(_) => {
                    if started && lists.is_empty() {
                        push(&mut job, "\n", look, &mut line_start, 0);
                        gap(&mut job, p);
                    } else if started && !line_start {
                        push(&mut job, "\n", look, &mut line_start, 0);
                    }
                    started = true;
                    line_start = true;
                    match tag {
                        Tag::Heading { .. } => look.strong = true,
                        Tag::CodeBlock(_) => look.code = true,
                        _ => {}
                    }
                }
                Tag::BlockQuote(_) => {
                    quotes += 1;
                    look.quote = true;
                }
                Tag::List(first) => {
                    if started && !line_start {
                        push(&mut job, "\n", look, &mut line_start, 0);
                    }
                    started = true;
                    lists.push(first);
                }
                Tag::Item => {
                    if !line_start {
                        push(&mut job, "\n", look, &mut line_start, quotes);
                    }
                    let depth = lists.len().saturating_sub(1).min(MAX_DEPTH);
                    let marker = match lists.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n = n.saturating_add(1);
                            m
                        }
                        _ => "• ".to_string(),
                    };
                    push(&mut job, &format!("{}{marker}", "    ".repeat(depth)), look, &mut line_start, quotes);
                }
                Tag::Emphasis => look.italic = true,
                Tag::Strong => look.strong = true,
                Tag::Strikethrough => look.muted = true,
                Tag::Link { dest_url, .. } => {
                    let to = web_link(&dest_url);
                    look.link = to.is_some();
                    link = Some((job.text.len(), to));
                }
                Tag::Image { .. } => {
                    // Its alt text is all that shows.
                    image += 1;
                    look.muted = true;
                }
                Tag::TableRow | Tag::TableHead if !line_start => push(&mut job, "\n", look, &mut line_start, quotes),
                Tag::TableCell if !line_start => push(&mut job, " · ", look, &mut line_start, quotes),
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock => {
                    look.strong = false;
                    look.code = false;
                }
                TagEnd::BlockQuote(_) => {
                    quotes = quotes.saturating_sub(1);
                    look.quote = quotes > 0;
                }
                TagEnd::List(_) => {
                    lists.pop();
                }
                TagEnd::Emphasis => look.italic = false,
                TagEnd::Strong => look.strong = false,
                TagEnd::Strikethrough => look.muted = false,
                TagEnd::Link => {
                    if let Some((from, Some(to))) = link.take() {
                        links.push((from..job.text.len(), to));
                    }
                    look.link = false;
                }
                TagEnd::Image => {
                    image = image.saturating_sub(1);
                    look.muted = image > 0;
                }
                _ => {}
            },
            Event::Text(t) => {
                let t = if look.code { t.trim_end_matches('\n').to_string() } else { t.to_string() };
                push(&mut job, &t, look, &mut line_start, quotes);
            }
            Event::Code(t) => push(&mut job, &t, Look { code: true, ..look }, &mut line_start, quotes),
            // HTML is shown as the text it is: nothing of it runs or loads.
            Event::Html(t) | Event::InlineHtml(t) => push(&mut job, &t, Look { muted: true, ..look }, &mut line_start, quotes),
            Event::FootnoteReference(t) => push(&mut job, &format!("[{t}]"), look, &mut line_start, quotes),
            Event::SoftBreak => push(&mut job, " ", look, &mut line_start, quotes),
            Event::HardBreak => push(&mut job, "\n", look, &mut line_start, quotes),
            Event::Rule => {
                if started {
                    push(&mut job, "\n", look, &mut line_start, 0);
                }
                started = true;
                push(&mut job, "———", Look { muted: true, ..Look::default() }, &mut line_start, quotes);
            }
            Event::TaskListMarker(done) => push(&mut job, if done { "[x] " } else { "[ ] " }, look, &mut line_start, quotes),
            Event::InlineMath(t) | Event::DisplayMath(t) => push(&mut job, &t, Look { code: true, ..look }, &mut line_start, quotes),
        }
    }
    // A trailing line break of the last block is not drawn as an empty line.
    while job.text.ends_with('\n') {
        let Some(last) = job.sections.last_mut() else { break };
        if last.byte_range.end.0.saturating_sub(last.byte_range.start.0) <= 1 {
            job.sections.pop();
        } else {
            last.byte_range.end = last.byte_range.end.saturating_sub(1);
        }
        job.text.pop();
    }
    if job.text.is_empty() {
        return plain(text, p, wrap);
    }
    Rendered { job, links }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only http and https with a host, no user part and no hidden character pass.
    #[test]
    fn only_a_web_address_that_shows_what_it_is_opens() {
        for ok in ["https://example.org/docs?a=1#x", "http://example.org", "https://example.org:8443/path"] {
            assert_eq!(web_link(ok).as_deref(), Some(ok), "{ok}");
        }
        for refused in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "mailto:someone@example.org",
            "ftp://example.org",
            "https://",
            "https:///path",
            "https://user:pass@example.org/",
            "https://example.org@evil.example/",
            "https://exa\u{200B}mple.org/",
            "https://example.org/\u{202E}gpj.exe",
            "https://example .org/",
            "https://example.org\\@evil.example",
            "https://\u{430}pple.com/",
            "https://example.org/\u{61C}x",
            "https://example.org/\u{E0041}",
            "data:text/html,hi",
            "//example.org",
        ] {
            assert!(web_link(refused).is_none(), "{refused:?}");
        }
    }

    /// A reply renders its Markdown at 13 on 19, images as alt text, HTML as text, links only when allowed.
    #[test]
    fn a_reply_renders_its_text_shows_images_as_alt_text_html_as_text_and_links_only_when_allowed() {
        let p = crate::ux::palette(false);
        let r = render(
            "Five **classes**: *Customer* and `Order`.\n\n- one\n- two\n\n![a diagram](https://example.org/x.png)\n\n<b>bold</b> [docs](https://example.org/docs) [run](javascript:alert(1))",
            p,
            240.0,
        );
        let text = &r.job.text;
        assert!(text.contains("Five classes: Customer and Order."), "{text:?}");
        assert!(text.contains("• one") && text.contains("• two"), "{text:?}");
        assert!(text.contains("a diagram") && !text.contains("x.png"), "an image is its alt text: {text:?}");
        assert!(text.contains("<b>bold</b>"), "HTML is plain text: {text:?}");
        assert!(text.contains("run") && !text.contains("javascript"), "{text:?}");
        assert_eq!(r.links.len(), 1, "only the web address is a link");
        let (range, to) = &r.links[0];
        assert_eq!((text.get(range.clone()), to.as_str()), (Some("docs"), "https://example.org/docs"));
        assert!(r.job.sections.iter().all(|s| s.format.line_height == Some(19.0) || s.format.line_height == Some(7.0)));
        // Nested past the budget: plain.
        let deep = format!("{}x", "> ".repeat(MAX_DEPTH + 5));
        assert_eq!(render(&deep, p, 240.0).job.text, deep);
    }
}
