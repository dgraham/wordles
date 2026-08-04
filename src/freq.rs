use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq)]
pub struct CharFreq {
    pub ch: char,
    pub count: usize,
}

impl Ord for CharFreq {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .count
            .cmp(&self.count)
            .then_with(|| self.ch.cmp(&other.ch))
    }
}

impl PartialOrd for CharFreq {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl CharFreq {
    pub fn frequencies(words: &[String]) -> Vec<Self> {
        let mut counts = HashMap::new();

        for word in words {
            for ch in word.chars() {
                *counts.entry(ch).or_insert(0) += 1;
            }
        }

        let mut counts = counts
            .into_iter()
            .map(|(ch, count)| Self { ch, count })
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
        let words = vec!["aaaaa".to_string(), "bbbba".to_string()];

        assert_eq!(
            CharFreq::frequencies(&words),
            vec![
                CharFreq { ch: 'a', count: 6 },
                CharFreq { ch: 'b', count: 4 }
            ]
        );
    }
}
