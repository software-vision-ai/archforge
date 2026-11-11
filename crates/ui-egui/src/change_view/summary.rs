//! The short summary above the Before and After cards (frames V1.1 and V3.3 of page 09): how many
//! values and members differ and which values, in whole sentences, worded while drawing from the two
//! sides held. What the cards don't hold yet is said plainly, so a card with no marks never reads as
//! "nothing changed".

use std::sync::Arc;

use archforge_mof::Universe;
use archforge_notations::differences::{CardField, ElementCard, ElementSides, ListKind};
use egui::{Galley, Rect, Stroke, StrokeKind, pos2};

use super::cards::worded;
use crate::Shell;
use crate::i18n::{fmt, tr, trn};
use crate::model_words::{kind_label, property_label};
use crate::settings::control;
use crate::ux::{self, Palette, types};

/// How many differences are written out before the rest are only counted, and how long a value
/// may run inside a sentence: the cards below hold it whole.
const MOST_SAID: usize = 3;
const LONGEST_VALUE: usize = 40;
/// A summary's inset, the middle of its headline, where its sentences start and the room under them.
const INSET: f32 = 20.0;
const HEAD_MID: f32 = 25.5;
const BODY_AT: f32 = 46.5;
const BELOW: f32 = 20.5;
/// The most rows of sentences a summary takes; hovering reads them all.
const MOST_ROWS: usize = 4;

/// A summary in the language shown: its headline and its sentences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Words {
    pub(super) head: String,
    pub(super) body: String,
}

/// What to say of `sides`. `listed` says the list of changes vouches that the element changed: a
/// failed list doesn't, so the summary then speaks only of what the cards hold.
pub(super) fn words(universe: &Universe, sides: &ElementSides, listed: bool) -> Words {
    let not_shown = tr("Relationships aren't listed on these cards yet.");
    let (Some(was), Some(is)) = (&sides.before, &sides.after) else {
        let (head, body) = if sides.before.is_none() {
            (tr("This element was added"), tr("It is only on the After side, so there is nothing to compare."))
        } else {
            (tr("This element was removed"), tr("It is only on the Before side, so there is nothing to compare."))
        };
        return Words { head, body: joined(&[body, not_shown]) };
    };
    let mut said = differences(universe, (was, is), &sides.fields);
    let owner = sides.lists.after.iter().any(|l| l.kind == ListKind::Ownership && l.rows.iter().any(|r| r.change.is_some()));
    let members = sides.lists.member_changes;
    if said.is_empty() && !owner && members == 0 {
        return if listed {
            Words { head: tr("Only what these cards don't show changed"), body: joined(&[tr("The cards show the same on both sides."), not_shown]) }
        } else {
            Words { head: tr("The cards show no difference"), body: not_shown }
        };
    }
    let count = said.len();
    said.truncate(MOST_SAID);
    if count > MOST_SAID {
        said.push(trn("{n} more value changed.", "{n} more values changed.", count.saturating_sub(MOST_SAID) as u64));
    }
    if owner {
        said.push(tr("The owner changed."));
    }
    said.push(not_shown);
    let member_head = trn("{n} member changed", "{n} members changed", members as u64);
    let value_head = trn("{n} value changed", "{n} values changed", count as u64);
    let head = match (count, members) {
        (0, 0) => tr("The owner changed"),
        (_, 0) => value_head,
        (0, _) => member_head,
        _ => fmt(&tr("{values} · {members}"), &[("values", &value_head), ("members", &member_head)]),
    };
    Words { head, body: joined(&said) }
}

/// One whole sentence for each thing that differs between the cards, in the order the cards show them.
fn differences(universe: &Universe, (was, is): (&ElementCard, &ElementCard), fields: &[CardField]) -> Vec<String> {
    let name = |card: &ElementCard| short(&card.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| tr("Unnamed")));
    let mut said = Vec::new();
    if was.name != is.name {
        said.push(from_to(&tr("The name changed from {from} to {to}."), &name(was), &name(is)));
    }
    if was.class != is.class {
        said.push(from_to(&tr("The kind changed from {from} to {to}."), &kind_label(universe, was.class), &kind_label(universe, is.class)));
    }
    said.extend(fields.iter().filter(|f| f.changed).map(|f| field_said(universe, f)));
    said
}

/// The sentence for one field that differs; a property only one side's class has is said to be there alone.
fn field_said(universe: &Universe, field: &CardField) -> String {
    let property = property_label(universe, field.prop).unwrap_or_else(|| tr("Unknown"));
    let named = |template: String| fmt(&template, &[("property", &property)]);
    match (&field.before, &field.after) {
        (Some(was), Some(is)) => {
            let value = |v| short(&worded(universe, field.prop, v));
            from_to(&named(tr("{property} changed from {from} to {to}.")), &value(was), &value(is))
        }
        (Some(_), None) => named(tr("{property} is only on the Before side.")),
        (None, _) => named(tr("{property} is only on the After side.")),
    }
}

/// `template` with both values in place. Each goes in once, so a value that happens to read
/// `{to}` or `{from}` stays as it is.
fn from_to(template: &str, from: &str, to: &str) -> String {
    let with_from = |part: &str| fmt(part, &[("from", from)]);
    template.split_once("{to}").map_or_else(|| with_from(template), |(start, end)| format!("{}{to}{}", with_from(start), with_from(end)))
}

/// `value` on one line and cut to [`LONGEST_VALUE`] characters, so one long text can't fill the summary.
fn short(value: &str) -> String {
    let flat = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= LONGEST_VALUE { flat } else { format!("{}…", flat.chars().take(LONGEST_VALUE).collect::<String>()) }
}

/// Whole sentences in a row: a space between them, none after a full-width stop.
fn joined(sentences: &[String]) -> String {
    let mut all = String::new();
    for sentence in sentences {
        if !all.is_empty() && !all.ends_with('。') {
            all.push(' ');
        }
        all.push_str(sentence);
    }
    all
}

/// The sentences laid out for a summary `width` wide.
pub(super) fn laid_out(paint: &egui::Painter, p: &Palette, words: &Words, width: f32) -> Arc<Galley> {
    ux::galley_rows(paint, &words.body, types::UI, p.text_secondary, Some((width - 2.0 * INSET).max(0.0)), MOST_ROWS)
}

/// How tall a summary is with `body` as its sentences.
pub(super) fn height(body: &Galley) -> f32 {
    BODY_AT + body.size().y + BELOW
}

/// Draws the summary in `area`, read whole on hover.
pub(super) fn draw(shell: &mut Shell, clip: &egui::Ui, p: &Palette, area: Rect, words: &Words, body: Arc<Galley>) {
    let paint = clip.painter().clone();
    let whole = format!("{}, {}", words.head, words.body);
    let _ = control(shell, clip, "workspace.change-view.summary", "status", area, false).on_hover_text(&whole).accessible_name(whole);
    paint.rect_filled(area, 10.0, p.surface);
    paint.rect_stroke(area, 10.0, Stroke::new(1.0, p.border), StrokeKind::Inside);
    let x = area.left() + INSET;
    ux::text_mid(&paint, x, area.top() + HEAD_MID, &words.head, types::LEAD_STRONG, p.text, Some((area.width() - 2.0 * INSET).max(0.0)));
    paint.galley(pos2(x, area.top() + BODY_AT), body, p.text_secondary);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value goes in once, whichever comes first in the language, and a value reading like a placeholder stays.
    #[test]
    fn values_go_in_once_in_either_order() {
        assert_eq!(from_to("from {from} to {to}.", "{to}", "{from}"), "from {to} to {from}.");
        assert_eq!(from_to("{to} after {from}", "a", "b"), "b after a");
        assert_eq!(from_to("only {from}", "a", "b"), "only a");
    }

    /// A long value is flattened and cut on a character, whatever its script; one at the limit stays whole.
    #[test]
    fn a_long_value_is_cut_on_a_character() {
        assert_eq!(short("two\n  lines"), "two lines");
        let limit = "é".repeat(LONGEST_VALUE);
        assert_eq!(short(&limit), limit);
        assert_eq!(short(&format!("{limit}é")), format!("{limit}…"));
    }

    /// Sentences are spaced, except after a full-width stop.
    #[test]
    fn sentences_join_as_their_script_does() {
        let join = |list: [&str; 2]| joined(&list.map(str::to_string));
        assert_eq!(join(["One.", "Two."]), "One. Two.");
        assert_eq!(join(["一。", "二。"]), "一。二。");
    }
}
