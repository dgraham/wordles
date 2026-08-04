use std::fmt;

const MISS: u16 = 0b00;
const HIT: u16 = 0b01;
const CONTAINS: u16 = 0b10;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Pattern(u16);

impl Pattern {
    pub fn new(guess: &str, candidate: &str) -> Self {
        Self(diff(guess, candidate))
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

fn diff(guess: &str, candidate: &str) -> u16 {
    guess.chars().enumerate().fold(0, |pattern, (index, ch)| {
        let value = if candidate.chars().nth(index) == Some(ch) {
            HIT
        } else if candidate.contains(ch) {
            CONTAINS
        } else {
            MISS
        };

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
    fn at_returns_the_value_at_each_position() {
        let pattern = Pattern::from(0b01_00_10_01_00);

        assert_eq!(pattern.at(0), 0b01);
        assert_eq!(pattern.at(1), 0b00);
        assert_eq!(pattern.at(2), 0b10);
        assert_eq!(pattern.at(3), 0b01);
        assert_eq!(pattern.at(4), 0b00);
    }
}
