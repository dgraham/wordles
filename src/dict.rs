use std::collections::HashSet;
use std::io::{Error, ErrorKind};

use crate::rule::{Position, Rule, RuleSet};
use fixedbitset::FixedBitSet;
use wide::u8x16;

const WORD_LENGTH: usize = 5;
const RECORD_LENGTH: usize = WORD_LENGTH + 1;
const BITS_PER_BLOCK: usize = usize::BITS as usize;

struct WordColumns {
    columns: [Vec<u8>; WORD_LENGTH],
}

impl WordColumns {
    fn new(records: &[[u8; RECORD_LENGTH]]) -> Self {
        let columns = std::array::from_fn(|col| records.iter().map(|record| record[col]).collect());
        Self { columns }
    }

    fn len(&self) -> usize {
        self.columns[0].len()
    }

    fn column_count(&self) -> usize {
        self.columns.len()
    }

    fn full_blocks(&self) -> impl Iterator<Item = usize> {
        (0..self.len() / 16 * 16).step_by(16)
    }

    fn full_block_end(&self) -> usize {
        self.len() / 16 * 16
    }

    fn tail(&self) -> std::ops::Range<usize> {
        self.full_block_end()..self.len()
    }

    fn block(&self, column: usize, start: usize) -> [u8; 16] {
        self.columns[column][start..start + 16]
            .try_into()
            .expect("full block contains 16 bytes")
    }

    fn word(&self, index: usize) -> [u8; WORD_LENGTH] {
        std::array::from_fn(|column| self.columns[column][index])
    }
}

pub struct Dictionary<'a> {
    contents: &'a str,
    columns: WordColumns,
}

impl<'a> TryFrom<&'a [u8]> for Dictionary<'a> {
    type Error = Error;

    fn try_from(contents: &'a [u8]) -> Result<Self, Self::Error> {
        if !contents.is_ascii() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Dictionary contents must be ASCII",
            ));
        }

        let (records, remainder) = contents.as_chunks::<RECORD_LENGTH>();
        if !remainder.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Records must contain six bytes",
            ));
        }
        if records.iter().any(|record| record[WORD_LENGTH] != b'\n') {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Every record must end with a newline",
            ));
        }
        // SAFETY: `contents.is_ascii()` above guarantees valid UTF-8.
        let text = unsafe { std::str::from_utf8_unchecked(contents) };
        let columns = WordColumns::new(records);

        Ok(Self {
            contents: text,
            columns,
        })
    }
}

impl<'a> Dictionary<'a> {
    pub fn words(&self) -> std::str::Lines<'a> {
        self.contents.lines()
    }

    pub fn get(&self, index: usize) -> Option<&'a str> {
        let start = index.checked_mul(RECORD_LENGTH)?;
        let end = start.checked_add(WORD_LENGTH)?;
        self.contents.get(start..end)
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.len() == 0
    }

    pub fn filter(&self, rules: &RuleSet) -> HashSet<&'a str> {
        let mut filter = BitFilter::new(&self.columns);
        let candidates = filter.filter(rules);
        let mut filtered = HashSet::with_capacity(candidates.count_ones(..));
        for word_index in candidates.ones() {
            filtered.insert(self.get(word_index).expect("candidate index is in bounds"));
        }

        filtered
    }
}

struct BitFilter<'a> {
    columns: &'a WordColumns,
    candidates: FixedBitSet,
}

impl<'a> BitFilter<'a> {
    fn new(columns: &'a WordColumns) -> Self {
        Self {
            columns,
            candidates: FixedBitSet::with_capacity(columns.len()),
        }
    }

    fn filter(&mut self, rules: &RuleSet) -> &FixedBitSet {
        self.candidates.insert_range(..);

        if rules.is_empty() {
            return &self.candidates;
        }

        self.apply_match(rules);
        self.apply_contains(rules);
        self.apply_once(rules);
        self.apply_none(rules);
        self.apply_tail(rules);

        &self.candidates
    }

    fn any_candidates_in_16(&self, start: usize) -> bool {
        debug_assert!(start + 16 <= self.candidates.len());
        self.candidates.as_slice()[start / BITS_PER_BLOCK] >> (start % BITS_PER_BLOCK)
            & usize::from(u16::MAX)
            != 0
    }

    fn clear_candidates_16(&mut self, start: usize, clear: u16) {
        debug_assert!(start + 16 <= self.candidates.len());
        self.candidates.as_mut_slice()[start / BITS_PER_BLOCK] &=
            !(usize::from(clear) << (start % BITS_PER_BLOCK));
    }

    fn apply_match(&mut self, rules: &RuleSet) {
        if self.candidates.is_clear() {
            return;
        }

        let constraints: Vec<_> = match_rules(rules).collect();
        if constraints.is_empty() {
            return;
        }

        let columns = self.columns;
        for word_index in columns.full_blocks() {
            let mut keep = u16::MAX;
            for &(character, position) in &constraints {
                if !character.is_ascii() {
                    keep = 0;
                    break;
                }
                let matches = u8x16::new(columns.block(position.index(), word_index))
                    .simd_eq(u8x16::splat(character as u8))
                    .to_bitmask() as u16;
                keep &= matches;
            }

            self.clear_candidates_16(word_index, !keep);
        }
    }

    fn apply_contains(&mut self, rules: &RuleSet) {
        if self.candidates.is_clear() {
            return;
        }

        let constraints: Vec<_> = contains_rules(rules).collect();
        if constraints.is_empty() {
            return;
        }

        let columns = self.columns;
        for word_index in columns.full_blocks() {
            let mut keep = u16::MAX;
            for &(character, position) in &constraints {
                if !character.is_ascii() {
                    keep = 0;
                    break;
                }
                let character = character as u8;
                let excluded_position = position.index();
                let mut occurs_elsewhere = 0u16;
                let mut occurs_at_excluded = 0u16;
                for position in 0..columns.column_count() {
                    let matches = u8x16::new(columns.block(position, word_index))
                        .simd_eq(u8x16::splat(character))
                        .to_bitmask() as u16;
                    if position == excluded_position {
                        occurs_at_excluded = matches;
                    } else {
                        occurs_elsewhere |= matches;
                    }
                }
                keep &= occurs_elsewhere & !occurs_at_excluded;
                if keep == 0 {
                    break;
                }
            }

            self.clear_candidates_16(word_index, !keep);
        }
    }

    fn apply_none(&mut self, rules: &RuleSet) {
        if self.candidates.is_clear() {
            return;
        }

        let constraints: Vec<u8> = none_rules(rules).collect();
        if constraints.is_empty() {
            return;
        }

        let columns = self.columns;
        for word_index in columns.full_blocks() {
            if !self.any_candidates_in_16(word_index) {
                continue;
            }

            let mut keep = u16::MAX;
            for &character in &constraints {
                let mut occurs = 0u16;
                for column in 0..columns.column_count() {
                    occurs |= u8x16::new(columns.block(column, word_index))
                        .simd_eq(u8x16::splat(character))
                        .to_bitmask() as u16;
                }
                keep &= !occurs;
            }

            self.clear_candidates_16(word_index, !keep);
        }
    }

    fn apply_once(&mut self, rules: &RuleSet) {
        if self.candidates.is_clear() {
            return;
        }

        let constraints: Vec<_> = once_rules(rules).collect();
        if constraints.is_empty() {
            return;
        }

        let columns = self.columns;
        for word_index in columns.full_blocks() {
            if !self.any_candidates_in_16(word_index) {
                continue;
            }

            let mut keep = u16::MAX;
            for &character in &constraints {
                if !character.is_ascii() {
                    keep = 0;
                    break;
                }
                let character = character as u8;
                let mut seen = 0u16;
                let mut multiple = 0u16;
                for column in 0..columns.column_count() {
                    let occurs = u8x16::new(columns.block(column, word_index))
                        .simd_eq(u8x16::splat(character))
                        .to_bitmask() as u16;
                    multiple |= seen & occurs;
                    seen |= occurs;
                }
                keep &= seen & !multiple;
            }

            self.clear_candidates_16(word_index, !keep);
        }
    }

    fn apply_tail(&mut self, rules: &RuleSet) {
        if self.candidates.is_clear() {
            return;
        }

        for word_index in self.columns.tail() {
            if self.candidates.contains(word_index) {
                let word = self.columns.word(word_index);
                let word = std::str::from_utf8(&word).expect("columns contain only ASCII bytes");
                if !rules.matches(word) {
                    self.candidates.remove(word_index);
                }
            }
        }
    }
}

fn match_rules(rules: &RuleSet) -> impl Iterator<Item = (char, Position)> + '_ {
    rules.rules().iter().filter_map(|rule| match *rule {
        Rule::Match(character, position) => Some((character, position)),
        _ => None,
    })
}

fn contains_rules(rules: &RuleSet) -> impl Iterator<Item = (char, Position)> + '_ {
    rules.rules().iter().filter_map(|rule| match *rule {
        Rule::Contains(character, position) => Some((character, position)),
        _ => None,
    })
}

fn none_rules(rules: &RuleSet) -> impl Iterator<Item = u8> + '_ {
    rules.rules().iter().filter_map(|rule| match *rule {
        Rule::None(character) if character.is_ascii() => Some(character as u8),
        _ => None,
    })
}

fn once_rules(rules: &RuleSet) -> impl Iterator<Item = char> + '_ {
    rules.rules().iter().filter_map(|rule| match *rule {
        Rule::Once(character) => Some(character),
        _ => None,
    })
}

impl AsRef<str> for Dictionary<'_> {
    fn as_ref(&self) -> &str {
        self.contents
    }
}

#[cfg(test)]
mod tests {
    use crate::rule::{Position, Rule, RuleSet};

    use super::{BitFilter, Dictionary, RECORD_LENGTH, WordColumns};

    const WORDS: &str = include_str!("../data/words");

    #[test]
    fn word_columns_separate_full_blocks_from_tail() {
        for (word_count, full_block_starts, tail_start) in
            [(0, 0, 0), (15, 0, 0), (16, 1, 16), (17, 1, 16), (32, 2, 32)]
        {
            let contents = "abcde\n".repeat(word_count);
            let (records, _) = contents.as_bytes().as_chunks::<RECORD_LENGTH>();
            let columns = WordColumns::new(records);

            assert_eq!(columns.len(), word_count);
            assert_eq!(
                columns.full_blocks().collect::<Vec<_>>(),
                (0..full_block_starts)
                    .map(|block| block * 16)
                    .collect::<Vec<_>>()
            );
            assert_eq!(columns.tail(), tail_start..word_count);
        }
    }

    fn position(value: u8) -> Position {
        Position::try_from(value).unwrap()
    }

    #[test]
    fn dictionary_borrows_and_validates_words() {
        let contents = String::from("slate\ncrate\n");
        let dictionary = Dictionary::try_from(contents.as_bytes()).unwrap();
        let borrowed_contents: &str = dictionary.as_ref();

        assert_eq!(borrowed_contents, contents);
        assert_eq!(borrowed_contents.as_ptr(), contents.as_ptr());
        assert_eq!(dictionary.words().collect::<Vec<_>>(), ["slate", "crate"]);
        assert_eq!(dictionary.len(), 2);
        assert!(!dictionary.is_empty());
        assert_eq!(dictionary.get(0), Some("slate"));
        assert_eq!(dictionary.get(1), Some("crate"));
        assert_eq!(dictionary.get(2), None);
        assert_eq!(dictionary.get(usize::MAX), None);
        assert!(Dictionary::try_from(&b"slate"[..]).is_err());
        assert!(Dictionary::try_from(&b"slate\r\n"[..]).is_err());
        assert!(Dictionary::try_from(&b"slatex"[..]).is_err());
        assert!(Dictionary::try_from("ébcde\n".as_bytes()).is_err());
    }

    #[test]
    fn filter_matches_ruleset_filter_for_builtin_and_custom_dictionaries() {
        let dictionary = Dictionary::try_from(WORDS.as_bytes()).unwrap();
        let words: Vec<_> = dictionary.words().collect();
        let mut rulesets = vec![
            RuleSet::default(),
            RuleSet::builder().matches(&["S1", "E5"]).build().unwrap(),
            RuleSet::builder().contains(&["A2"]).build().unwrap(),
            RuleSet::builder().contains(&["A1"]).build().unwrap(),
            RuleSet::builder().none(&["E"]).build().unwrap(),
            RuleSet::builder().none(&["A", "T"]).build().unwrap(),
            RuleSet::builder().once(&["E"]).build().unwrap(),
            RuleSet::builder().once(&["A", "E"]).build().unwrap(),
            RuleSet::builder()
                .matches(&["S1", "E5"])
                .contains(&["A2"])
                .none(&["R"])
                .once(&["E"])
                .build()
                .unwrap(),
        ];
        rulesets.extend([
            [Rule::Match('é', position(1))].into_iter().collect(),
            [Rule::Contains('a', position(5))].into_iter().collect(),
            [Rule::Contains('é', position(1))].into_iter().collect(),
            [Rule::None('é')].into_iter().collect(),
            [Rule::Once('é')].into_iter().collect(),
        ]);
        for rules in rulesets {
            let filtered = dictionary.filter(&rules);
            assert_eq!(filtered, rules.filter(&words));
        }

        let empty_dictionary = Dictionary::try_from(&b""[..]).unwrap();
        let empty_rules = RuleSet::default();
        assert!(empty_dictionary.filter(&empty_rules).is_empty());

        let custom_dictionary =
            Dictionary::try_from(&b"abaca\nbaaaa\nslate\ncrate\nplumb\n"[..]).unwrap();
        let custom_words: Vec<_> = custom_dictionary.words().collect();
        for rules in [
            RuleSet::builder().matches(&["S1"]).build().unwrap(),
            RuleSet::builder().contains(&["A1"]).build().unwrap(),
            RuleSet::builder().once(&["A"]).build().unwrap(),
            RuleSet::builder().none(&["A"]).build().unwrap(),
        ] {
            let filtered = custom_dictionary.filter(&rules);
            assert_eq!(filtered, rules.filter(&custom_words));
        }
    }

    #[test]
    fn bit_filter_reuses_candidates_across_rule_sets() {
        let dictionary = Dictionary::try_from(WORDS.as_bytes()).unwrap();
        let words: Vec<_> = dictionary.words().collect();
        let rulesets = [
            RuleSet::builder().matches(&["S1", "C1"]).build().unwrap(),
            RuleSet::default(),
            RuleSet::builder().once(&["A"]).build().unwrap(),
        ];
        let mut bit_filter = BitFilter::new(&dictionary.columns);

        for rules in rulesets {
            let filtered: std::collections::HashSet<_> = bit_filter
                .filter(&rules)
                .ones()
                .map(|index| dictionary.get(index).unwrap())
                .collect();
            assert_eq!(filtered, rules.filter(&words));
        }
    }

    #[test]
    fn filter_combines_conflicting_match_rules() {
        let dictionary = Dictionary::try_from(WORDS.as_bytes()).unwrap();
        let rules = RuleSet::builder().matches(&["S1", "C1"]).build().unwrap();

        assert!(dictionary.filter(&rules).is_empty());
    }

    #[test]
    fn contains_conflicts_filter_to_no_candidates() {
        let dictionary = Dictionary::try_from(WORDS.as_bytes()).unwrap();
        let same_character = RuleSet::builder()
            .contains(&["A1", "A2", "A3", "A4", "A5"])
            .build()
            .unwrap();
        let competing_characters = RuleSet::builder()
            .contains(&["A1", "B1", "C1", "D1", "E1"])
            .build()
            .unwrap();

        assert!(dictionary.filter(&same_character).is_empty());
        assert!(dictionary.filter(&competing_characters).is_empty());
    }

    #[test]
    fn contains_filter_excludes_the_specified_position() {
        let dictionary = Dictionary::try_from(&b"abaca\nbaaaa\nslate\ncrate\nplumb\n"[..]).unwrap();
        let rules = RuleSet::builder().contains(&["A1"]).build().unwrap();
        assert_eq!(
            dictionary.filter(&rules),
            ["baaaa", "slate", "crate"].into_iter().collect()
        );
    }

    #[test]
    fn none_filter_preserves_candidates_matching_rules() {
        let dictionaries = [
            Dictionary::try_from(WORDS.as_bytes()).unwrap(),
            Dictionary::try_from(&b"abaca\nbaaaa\nslate\ncrate\nplumb\n"[..]).unwrap(),
        ];
        let mut rulesets = vec![
            RuleSet::default(),
            RuleSet::builder().none(&["E"]).build().unwrap(),
            RuleSet::builder().none(&["A", "T"]).build().unwrap(),
            RuleSet::builder().none(&["Z"]).build().unwrap(),
        ];
        rulesets.push([Rule::None('é')].into_iter().collect());

        for dictionary in dictionaries {
            let mut wide_filter = BitFilter::new(&dictionary.columns);
            let mut initial = BitFilter::new(&dictionary.columns).candidates;
            initial.remove(0);

            for rules in &rulesets {
                let expected: Vec<_> = initial
                    .ones()
                    .filter(|&word_index| {
                        rules.matches(
                            dictionary
                                .get(word_index)
                                .expect("candidate index is in bounds"),
                        )
                    })
                    .collect();
                wide_filter.candidates = initial.clone();
                wide_filter.apply_none(rules);
                wide_filter.apply_tail(rules);

                assert_eq!(wide_filter.candidates.ones().collect::<Vec<_>>(), expected);
            }
        }
    }

    #[test]
    fn once_filter_preserves_candidates_matching_rules() {
        let dictionaries = [
            Dictionary::try_from(WORDS.as_bytes()).unwrap(),
            Dictionary::try_from(&b"abaca\nbaaaa\nslate\ncrate\nplumb\n"[..]).unwrap(),
        ];
        let mut rulesets = vec![
            RuleSet::default(),
            RuleSet::builder().once(&["A"]).build().unwrap(),
            RuleSet::builder().once(&["E", "R"]).build().unwrap(),
            RuleSet::builder().once(&["A", "E"]).build().unwrap(),
        ];
        rulesets.push([Rule::Once('é')].into_iter().collect());

        for dictionary in dictionaries {
            let mut wide_filter = BitFilter::new(&dictionary.columns);
            let mut initial = BitFilter::new(&dictionary.columns).candidates;
            initial.remove(0);

            for rules in &rulesets {
                let expected: Vec<_> = initial
                    .ones()
                    .filter(|&word_index| {
                        rules.matches(
                            dictionary
                                .get(word_index)
                                .expect("candidate index is in bounds"),
                        )
                    })
                    .collect();
                wide_filter.candidates = initial.clone();
                wide_filter.apply_once(rules);
                wide_filter.apply_tail(rules);

                assert_eq!(wide_filter.candidates.ones().collect::<Vec<_>>(), expected);
            }
        }

        let dictionary = Dictionary::try_from(&b"abaca\nbaaaa\nslate\ncrate\nplumb\n"[..]).unwrap();
        let rules = RuleSet::builder().once(&["A"]).build().unwrap();
        assert_eq!(
            dictionary.filter(&rules),
            ["slate", "crate"].into_iter().collect()
        );
    }
}
