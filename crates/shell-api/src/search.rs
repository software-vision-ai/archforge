//! How a search or a filter matches what a person types: the typed words folded once by
//! craft-i18n's `Folder`, the texts they are matched against folded the same way, and where a
//! word may start in a folded text, never inside a letter. Home's search, a project's, the
//! Models, Diagrams and Versions filters, the version list and the stereotype picker all match
//! through it (`archforge_ui_egui::home_search` re-exports it under its old path).

use craft_i18n::{Folded, Folder};

/// What was typed into a search or a filter, folded once by craft-i18n's `Folder`: capitals,
/// accents, `ß` against `ss` and compatibility forms (full-width letters, ligatures) count for
/// nothing. Every name search a person types into matches through it, so Home's search, a
/// project's, the Models, Diagrams and Versions filters, the version list and the stereotype
/// picker agree.
#[derive(Debug, Clone)]
pub struct Typed {
    /// Folded once and never again: a second fold can reorder marks a dropped character kept
    /// apart, which a name folded once keeps in place.
    words: Vec<String>,
}

impl Typed {
    /// Each word of `query` on its own: a text answers when it holds every one. The query is
    /// folded before it is split, so a ligature that stands for several words (`ﷻ`) gives each
    /// of them, and a word of invisible characters only (a pasted zero-width space) is none.
    pub fn words(query: &str) -> Typed {
        Typed { words: folder().key(query).split_whitespace().map(str::to_owned).collect() }
    }

    /// `text` as one piece, spaces and all; nothing when it folds to nothing.
    pub fn phrase(text: &str) -> Typed {
        Typed { words: Some(folder().key(text)).filter(|w| !w.is_empty()).into_iter().collect() }
    }

    /// `text` as one piece, folded and then trimmed, so spaces kept by invisible characters
    /// around it (`\u{200B} prufen \u{200B}`) go too: the Models and Diagrams filters.
    pub fn trimmed(text: &str) -> Typed {
        Typed { words: Some(folder().key(text).trim()).filter(|w| !w.is_empty()).map(str::to_owned).into_iter().collect() }
    }

    /// Whether nothing was typed, so everything answers.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// The first word folded, to rank names by; none when nothing was typed.
    pub fn first(&self) -> Option<&str> {
        self.words.first().map(String::as_str)
    }

    /// Whether every word is in one of `texts`, already folded by [`fold`].
    pub fn in_folded(&self, texts: &[&Folded]) -> bool {
        self.words.iter().all(|w| texts.iter().any(|t| starts(t, w).next().is_some()))
    }

    /// Whether every word is in one of `texts`; true at once, folding nothing, when nothing was
    /// typed.
    pub fn in_any(&self, texts: &[&str]) -> bool {
        self.words.iter().all(|w| texts.iter().any(|t| starts(&fold(t), w).next().is_some()))
    }
}

/// The folding for the interface's language. Accents count for nothing, so the language changes
/// no match: only with accents counted does Turkish set its dotted and dotless i apart.
fn folder() -> Folder {
    Folder::new(crate::i18n::language())
}

/// `text` folded as [`Typed`] folds what is typed, for a text matched again and again.
pub fn fold(text: &str) -> Folded {
    folder().fold(text)
}

/// Whether `next` goes on the letter before it rather than starting one: a combining mark, or a
/// Hangul vowel or final consonant that continues `prev`'s syllable.
pub fn joins(prev: Option<char>, next: char) -> bool {
    use icu_properties::props::{GraphemeExtend, HangulSyllableType as H};
    let hangul = icu_properties::CodePointMapData::<H>::new();
    let syllable = |p| matches!((hangul.get(p), hangul.get(next)), (H::LeadingJamo, H::VowelJamo) | (H::VowelJamo | H::LVSyllable, H::TrailingJamo));
    icu_properties::CodePointSetData::new::<GraphemeExtend>().contains(next) || prev.is_some_and(syllable)
}

/// The folded offsets where the folded `word` starts in `name`, overlapping ones too, on the
/// boundaries `Folded::find` matches on: never inside a letter, where a Hangul syllable part
/// continues one only within one source character. Every search matches and ranks through it.
pub fn starts<'a>(name: &'a Folded, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    let text = name.as_str();
    let edge = move |at: usize| {
        let Some(next) = text.get(at..).and_then(|t| t.chars().next()) else { return true };
        let span = |from: usize, c: char| name.source_range(from..from + c.len_utf8());
        let prev = text.get(..at).and_then(|t| t.chars().next_back()).filter(|p| span(at.saturating_sub(p.len_utf8()), *p) == span(at, next));
        !joins(prev, next)
    };
    let found = move |at: usize| text.get(at..).is_some_and(|t| t.starts_with(word));
    // A word that starts with a mark, its space split or trimmed off (`゛` folds to a space and
    // U+3099), starts where a space carries that mark or where the name itself starts (`ﾞDraft`);
    // never on a letter, so `ハ` keeps out of `バ`.
    let carried = move |at: usize| text.get(..at).and_then(|t| t.chars().next_back()).is_none_or(char::is_whitespace);
    text.char_indices().map(|(at, _)| at).filter(move |&at| found(at) && (edge(at) || carried(at)) && edge(at + word.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What folds to nothing is not typed, and a ligature of several words gives each of them.
    #[test]
    fn invisible_words_are_dropped_and_a_ligature_gives_its_words() {
        assert!(Typed::phrase("\u{200B}").is_empty() && Typed::words("\u{AD} \u{200B}").is_empty());
        assert_eq!(Typed::words("\u{200B} Order").first(), Some("order"));
        // ﷻ folds to two Arabic words; a name holding both apart, with a comma, is found.
        assert!(Typed::words("\u{FDFB}").in_any(&["جل، جلاله"]) && !Typed::words("\u{FDFB}").in_any(&["جل"]));
        // Folded once, as the name is: a second fold would put the nukta before the virama.
        let marks = "क\u{94D}\u{200B}\u{93C}";
        assert!(Typed::trimmed(marks).in_any(&[marks]) && Typed::words(marks).in_any(&[marks]));
        // ゛ folds to a space and a voicing mark: the mark, split or trimmed off its space, finds it.
        assert!(Typed::words("Draft ゛").in_any(&["Draft ゛"]) && Typed::trimmed("゛").in_any(&["Draft ゛"]));
        assert!(!Typed::words("゛").in_any(&["バ"]));
    }
}
