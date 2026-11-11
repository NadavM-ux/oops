//! Keyboard-layout conversion between English (US QWERTY) and Hebrew (SI-1452).
//!
//! Pure logic, no OS calls. Every character is re-interpreted as "the same physical
//! key, typed on the other layout": `akuo` -> `שלום`, `,usv` -> `תודה`.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    /// Text was typed on the English layout but meant to be Hebrew.
    EnToHe,
    /// Text was typed on the Hebrew layout but meant to be English.
    HeToEn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Lang {
    English,
    Hebrew,
}

impl Direction {
    pub fn reverse(self) -> Self {
        match self {
            Direction::EnToHe => Direction::HeToEn,
            Direction::HeToEn => Direction::EnToHe,
        }
    }

    /// The language the text is in after the conversion.
    pub fn target(self) -> Lang {
        match self {
            Direction::EnToHe => Lang::Hebrew,
            Direction::HeToEn => Lang::English,
        }
    }
}

/// `(what the key types on the US layout, what the same key types on the Hebrew layout)`.
/// Keys that type the same character on both layouts (digits, brackets, shifted symbols)
/// are intentionally absent: they never need converting.
pub const KEY_PAIRS: [(char, char); 32] = [
    ('`', ';'),
    ('q', '/'),
    ('w', '\''),
    ('e', 'ק'),
    ('r', 'ר'),
    ('t', 'א'),
    ('y', 'ט'),
    ('u', 'ו'),
    ('i', 'ן'),
    ('o', 'ם'),
    ('p', 'פ'),
    ('a', 'ש'),
    ('s', 'ד'),
    ('d', 'ג'),
    ('f', 'כ'),
    ('g', 'ע'),
    ('h', 'י'),
    ('j', 'ח'),
    ('k', 'ל'),
    ('l', 'ך'),
    (';', 'ף'),
    ('\'', ','),
    ('z', 'ז'),
    ('x', 'ס'),
    ('c', 'ב'),
    ('v', 'ה'),
    ('b', 'נ'),
    ('n', 'מ'),
    ('m', 'צ'),
    (',', 'ת'),
    ('.', 'ץ'),
    ('/', '.'),
];

pub fn is_hebrew_letter(c: char) -> bool {
    ('\u{05D0}'..='\u{05EA}').contains(&c)
}

/// Maps one character to the character the same key produces on the other layout.
pub fn map_char(c: char, dir: Direction) -> Option<char> {
    match dir {
        Direction::EnToHe => {
            let key = c.to_ascii_lowercase();
            KEY_PAIRS.iter().find(|p| p.0 == key).map(|p| p.1)
        }
        Direction::HeToEn => KEY_PAIRS.iter().find(|p| p.1 == c).map(|p| p.0),
    }
}

/// Letters of the script we convert *from*.
fn is_source_letter(c: char, dir: Direction) -> bool {
    match dir {
        Direction::EnToHe => c.is_ascii_alphabetic(),
        Direction::HeToEn => is_hebrew_letter(c),
    }
}

/// Letters that prove a word was already typed correctly in the *target* script.
/// Latin capitals don't count for Hebrew->English: Shift+key on the Hebrew layout
/// types them, so they show up inside Hebrew-layout gibberish ("Hקךךם" = "Hello").
fn is_target_letter(c: char, dir: Direction) -> bool {
    match dir {
        Direction::EnToHe => is_hebrew_letter(c),
        Direction::HeToEn => c.is_ascii_lowercase(),
    }
}

#[derive(Clone, Copy)]
enum WordMode {
    All,
    LettersOnly,
    Keep,
}

/// Converts `text` word by word.
///
/// * Words made of source-script letters are converted completely, punctuation included
///   (`,` is `ת` on the Hebrew layout, so `ac,` -> `שבת`).
/// * Words that already contain target-script letters keep their punctuation and only
///   have stray source letters converted, so `שלום,` keeps its comma.
/// * Words without letters (`.`, `123`) are converted only when nothing in the text is
///   already in the target script.
pub fn convert(text: &str, dir: Direction) -> String {
    let target_anywhere = text.chars().any(|c| is_target_letter(c, dir));
    let mut out = String::with_capacity(text.len() * 2);
    let mut word: Vec<char> = Vec::new();

    let flush = |word: &mut Vec<char>, out: &mut String| {
        if word.is_empty() {
            return;
        }
        let has_src = word.iter().any(|&c| is_source_letter(c, dir));
        let has_tgt = word.iter().any(|&c| is_target_letter(c, dir));
        let mode = match (has_src, has_tgt) {
            (true, false) => WordMode::All,
            (true, true) => WordMode::LettersOnly,
            (false, false) if !target_anywhere => WordMode::All,
            _ => WordMode::Keep,
        };
        for &c in word.iter() {
            let mapped = match mode {
                WordMode::All => map_char(c, dir),
                WordMode::LettersOnly if is_source_letter(c, dir) => map_char(c, dir),
                _ => None,
            };
            out.push(mapped.unwrap_or(c));
        }
        word.clear();
    };

    for c in text.chars() {
        if c.is_whitespace() {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Decides which way a selected piece of text should be converted: by majority script.
pub fn detect_direction(text: &str) -> Option<Direction> {
    let (mut latin, mut hebrew) = (0usize, 0usize);
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            latin += 1;
        } else if is_hebrew_letter(c) {
            hebrew += 1;
        }
    }
    match (latin, hebrew) {
        (0, 0) => None,
        (l, h) if h > l => Some(Direction::HeToEn),
        _ => Some(Direction::EnToHe),
    }
}

/// A minimal edit at the end of the text field: delete `delete` characters
/// (`removed`), then type `insert`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub delete: usize,
    pub insert: String,
    pub removed: String,
    pub dir: Direction,
}

/// Plans the fix for text the user just typed (the keystroke buffer, caret at the end).
///
/// The direction comes from the most recent letter. Only the tail typed in the wrong
/// script is touched: in `"Hello שלום nv"` just `nv` becomes `מה`.
pub fn plan_typed(buffer: &str) -> Option<Edit> {
    let chars: Vec<char> = buffer.chars().collect();
    let last_letter = chars
        .iter()
        .rev()
        .find(|c| c.is_ascii_alphabetic() || is_hebrew_letter(**c))?;
    let dir = if last_letter.is_ascii_alphabetic() {
        Direction::EnToHe
    } else {
        Direction::HeToEn
    };

    let mut start = chars
        .iter()
        .rposition(|&c| is_target_letter(c, dir))
        .map(|i| i + 1)
        .unwrap_or(0);
    if start > 0 {
        // Punctuation glued to the correctly-typed word before us belongs to that word.
        while start < chars.len() && !chars[start].is_whitespace() && !is_source_letter(chars[start], dir) {
            start += 1;
        }
        while start < chars.len() && chars[start].is_whitespace() {
            start += 1;
        }
    }

    let segment: Vec<char> = chars[start..].to_vec();
    let seg_text: String = segment.iter().collect();
    let converted: Vec<char> = convert(&seg_text, dir).chars().collect();
    let common = segment
        .iter()
        .zip(converted.iter())
        .take_while(|(a, b)| a == b)
        .count();
    if common == segment.len() && common == converted.len() {
        return None;
    }
    Some(Edit {
        delete: segment.len() - common,
        insert: converted[common..].iter().collect(),
        removed: segment[common..].iter().collect(),
        dir,
    })
}

/// Plans the fix for selected text: returns the replacement and its direction.
pub fn plan_selection(selection: &str) -> Option<(String, Direction)> {
    let dir = detect_direction(selection)?;
    let out = convert(selection, dir);
    if out == selection {
        None
    } else {
        Some((out, dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    #[test]
    fn classic_examples() {
        assert_eq!(convert("akuo", EnToHe), "שלום");
        assert_eq!(convert("AKUO", EnToHe), "שלום");
        assert_eq!(convert("Akuo", EnToHe), "שלום");
        assert_eq!(convert("nv akunl?", EnToHe), "מה שלומך?");
        assert_eq!(convert(",usv rcv", EnToHe), "תודה רבה");
        assert_eq!(convert("ac,", EnToHe), "שבת");
        assert_eq!(convert("dwhrpv", EnToHe), "ג'ירפה");
        assert_eq!(convert("akuo' nv akunl?", EnToHe), "שלום, מה שלומך?");
        assert_eq!(convert("vhh/", EnToHe), "היי.");
    }

    #[test]
    fn hebrew_to_english() {
        assert_eq!(convert("ןמנםס", HeToEn), "inbox");
        assert_eq!(convert("Hקךךם", HeToEn), "Hello");
        assert_eq!(convert("'ישא,ד ופ?", HeToEn), "what's up?");
        assert_eq!(convert("/וןא", HeToEn), "quit");
    }

    #[test]
    fn every_key_round_trips() {
        for (en, he) in KEY_PAIRS {
            assert_eq!(map_char(en, EnToHe), Some(he));
            assert_eq!(map_char(he, HeToEn), Some(en));
        }
        let all_hebrew: Vec<char> = KEY_PAIRS.iter().map(|p| p.1).filter(|c| is_hebrew_letter(*c)).collect();
        assert_eq!(all_hebrew.len(), 27, "all 22 letters + 5 final forms");
    }

    #[test]
    fn neutral_characters_survive() {
        assert_eq!(convert("akuo 123!", EnToHe), "שלום 123!");
        assert_eq!(convert("a\tb\nc", EnToHe), "ש\tנ\nב");
        assert_eq!(convert("", EnToHe), "");
        assert_eq!(convert("(akuo)", EnToHe), "(שלום)");
    }

    #[test]
    fn mixed_words_keep_their_punctuation() {
        assert_eq!(convert("שלום, akuo", EnToHe), "שלום, שלום");
        assert_eq!(convert("שלום akuo", EnToHe), "שלום שלום");
        assert_eq!(convert("hello יקךךם", HeToEn), "hello hello");
    }

    #[test]
    fn detects_direction_by_majority() {
        assert_eq!(detect_direction("akuo"), Some(EnToHe));
        assert_eq!(detect_direction("ןמנםס"), Some(HeToEn));
        assert_eq!(detect_direction("Hקךךם"), Some(HeToEn));
        assert_eq!(detect_direction("שלום akuo"), Some(EnToHe));
        assert_eq!(detect_direction("123 !?"), None);
    }

    fn edit(delete: usize, insert: &str, removed: &str, dir: Direction) -> Option<Edit> {
        Some(Edit { delete, insert: insert.into(), removed: removed.into(), dir })
    }

    #[test]
    fn typed_whole_buffer() {
        assert_eq!(plan_typed("akuo"), edit(4, "שלום", "akuo", EnToHe));
        assert_eq!(plan_typed("akuo "), edit(5, "שלום ", "akuo ", EnToHe));
        assert_eq!(plan_typed("ןמנםס"), edit(5, "inbox", "ןמנםס", HeToEn));
    }

    #[test]
    fn typed_only_the_wrong_tail() {
        assert_eq!(plan_typed("Hello שלום nv"), edit(2, "מה", "nv", EnToHe));
        assert_eq!(plan_typed("שלום, akuo"), edit(4, "שלום", "akuo", EnToHe));
        assert_eq!(plan_typed("hello 'ישא"), edit(4, "what", "'ישא", HeToEn));
        assert_eq!(plan_typed("שלום ,usv"), edit(4, "תודה", ",usv", EnToHe));
    }

    #[test]
    fn typed_keeps_unchanged_prefix() {
        assert_eq!(plan_typed("123akuo"), edit(4, "שלום", "akuo", EnToHe));
    }

    #[test]
    fn typed_nothing_to_do() {
        assert_eq!(plan_typed(""), None);
        assert_eq!(plan_typed("123 !!"), None);
        assert_eq!(plan_typed("   "), None);
    }

    #[test]
    fn selection_plans() {
        assert_eq!(plan_selection("akuo"), Some(("שלום".into(), EnToHe)));
        assert_eq!(plan_selection("שלום, akuo"), Some(("שלום, שלום".into(), EnToHe)));
        assert_eq!(plan_selection("ןמנםס"), Some(("inbox".into(), HeToEn)));
        assert_eq!(plan_selection("12:30"), None);
    }

    #[test]
    fn convert_then_back_is_identity_for_lowercase_gibberish() {
        let samples = ["akuo nv akunl?", ",usv rcv", "dwhrpv", "ac,", "vhh/"];
        for s in samples {
            let he = convert(s, EnToHe);
            assert_eq!(convert(&he, HeToEn), s, "{s} -> {he}");
        }
    }
}
