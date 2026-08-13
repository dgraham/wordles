use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub struct CharFreq {
    pub ch: char,
    pub count: usize,
    pub percentage: f64,
}

impl Ord for CharFreq {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .count
            .cmp(&self.count)
            .then_with(|| self.ch.cmp(&other.ch))
            .then_with(|| self.percentage.total_cmp(&other.percentage))
    }
}

impl Eq for CharFreq {}

impl PartialOrd for CharFreq {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl CharFreq {
    pub fn new(ch: char, count: usize, total: usize) -> Self {
        Self {
            ch,
            count,
            percentage: count as f64 / total as f64,
        }
    }

    pub fn frequencies(words: &[&str]) -> Vec<Self> {
        let mut counts = HashMap::new();
        let total = words.len() * 5;

        for word in words {
            for ch in word.chars() {
                *counts.entry(ch).or_default() += 1;
            }
        }

        let mut counts = counts
            .into_iter()
            .map(|(ch, count)| Self::new(ch, count, total))
            .collect::<Vec<_>>();
        counts.sort();
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::CharFreq;

    #[test]
    fn frequencies_returns_counts_in_descending_order() {
        let words = vec!["aaaaa", "bbbba"];

        assert_eq!(
            CharFreq::frequencies(&words),
            vec![CharFreq::new('a', 6, 10), CharFreq::new('b', 4, 10),]
        );
    }
}
