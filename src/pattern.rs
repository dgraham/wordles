use std::fmt;

const MISS: u16 = 0b00;
const HIT: u16 = 0b01;
const CONTAINS: u16 = 0b10;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Pattern(u16);

impl Pattern {
    pub fn new(guess: &str, solution: &str) -> Self {
        Self(diff(guess, solution))
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn at(self, index: usize) -> u16 {
        match index {
            0..=4 => (self.0 >> (8 - index * 2)) & 0b11,
            _ => MISS,
        }
    }

    pub fn emoji(self) -> String {
        (0..5)
            .map(|index| square(self.at(index)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn highlight(self, word: &str) -> String {
        word.chars()
            .enumerate()
            .map(|(index, ch)| {
                let color = match self.at(index) {
                    HIT => "\x1b[42;30;1m",
                    CONTAINS => "\x1b[43;30;1m",
                    _ => "\x1b[100;37;1m",
                };
                format!("{color} {ch} \x1b[0m")
            })
            .collect()
    }
}

impl From<u16> for Pattern {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

fn diff(guess: &str, solution: &str) -> u16 {
    let mut guess_chars = [None; 5];
    for (slot, ch) in guess_chars.iter_mut().zip(guess.chars()) {
        *slot = Some(ch);
    }

    let mut solution_chars = [None; 5];
    for (slot, ch) in solution_chars.iter_mut().zip(solution.chars()) {
        *slot = Some(ch);
    }

    let mut solution_counts = [0; 5];
    for solution_char in solution.chars() {
        for (index, guess_char) in guess_chars.iter().enumerate() {
            if *guess_char == Some(solution_char) {
                solution_counts[index] += 1;
            }
        }
    }

    let mut results = [MISS; 5];
    for index in 0..5 {
        if guess_chars[index].is_some_and(|ch| solution_chars[index] == Some(ch)) {
            results[index] = HIT;
        }
    }

    for index in 0..5 {
        let Some(ch) = guess_chars[index] else {
            continue;
        };
        if results[index] == HIT {
            continue;
        }

        let matched_count = (0..5)
            .filter(|&matched| guess_chars[matched] == Some(ch) && results[matched] != MISS)
            .count();
        if solution_counts[index] > matched_count {
            results[index] = CONTAINS;
        }
    }

    results
        .iter()
        .enumerate()
        .fold(0, |pattern, (index, &value)| {
            pattern | (value << (8 - index * 2))
        })
}

fn square(value: u16) -> &'static str {
    match value {
        HIT => "🟩",
        CONTAINS => "🟨",
        _ => "⬜️",
    }
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    #[test]
    fn diff_encodes_hits_and_misses() {
        assert_eq!(
            Pattern::new("abcde", "axcye"),
            Pattern::from(0b01_00_01_00_01)
        );
    }

    #[test]
    fn diff_encodes_contained_letters() {
        assert_eq!(
            Pattern::new("abcde", "ezzzz"),
            Pattern::from(0b00_00_00_00_10)
        );
        assert_eq!(
            Pattern::new("abcde", "abcde"),
            Pattern::from(0b01_01_01_01_01)
        );
    }

    #[test]
    fn diff_counts_duplicate_letters_after_hits() {
        assert_eq!(
            Pattern::new("allee", "apple"),
            Pattern::from(0b01_10_00_00_01)
        );
    }

    #[test]
    fn diff_counts_duplicate_unicode_letters_after_hits() {
        assert_eq!(
            Pattern::new("ééabc", "éxxyz"),
            Pattern::from(0b01_00_00_00_00)
        );
    }

    #[test]
    fn diff_ignores_characters_past_the_fifth() {
        assert_eq!(
            Pattern::new("abcdef", "abcdef"),
            Pattern::from(0b01_01_01_01_01)
        );
    }

    #[test]
    fn at_returns_the_value_at_each_position() {
        let pattern = Pattern::from(0b01_00_10_01_00);

        assert_eq!(pattern.at(0), 0b01);
        assert_eq!(pattern.at(1), 0b00);
        assert_eq!(pattern.at(2), 0b10);
        assert_eq!(pattern.at(3), 0b01);
        assert_eq!(pattern.at(4), 0b00);
    }
}
