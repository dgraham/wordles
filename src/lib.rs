use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::error::Error;

use lmdb::{Database, Transaction};

pub mod rule;

pub const MISS: u16 = 0b00;
pub const HIT: u16 = 0b01;
pub const CONTAINS: u16 = 0b10;

pub struct Ranking {
    pub word: String,
    pub groups: usize,
    pub average: f64,
    pub max: usize,
}

impl Ord for Ranking {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .groups
            .cmp(&self.groups)
            .then_with(|| self.average.total_cmp(&other.average))
            .then_with(|| self.max.cmp(&other.max))
            .then_with(|| self.word.cmp(&other.word))
    }
}

impl PartialOrd for Ranking {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Ranking {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Ranking {}

pub fn diff(guess: &str, candidate: &str) -> u16 {
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

pub fn rank_from_cache<T: Transaction>(
    txn: &T,
    db: Database,
    candidates: &HashSet<String>,
) -> Result<Vec<Ranking>, Box<dyn Error>> {
    let mut rankings = Vec::new();

    for word in candidates {
        let patterns = std::str::from_utf8(txn.get(db, word)?)?;
        let mut groups = 0;
        let mut max = 0;
        let mut sum = 0;

        for pattern in patterns.split(',').filter(|pattern| !pattern.is_empty()) {
            let key = format!("{word}:{pattern}");
            let words = std::str::from_utf8(txn.get(db, &key)?)?;
            let count = words
                .split(',')
                .filter(|candidate| candidates.contains(*candidate))
                .count();

            if count > 0 {
                max = max.max(count);
                sum += count;
                groups += 1;
            }
        }

        if groups > 0 {
            rankings.push(Ranking {
                word: word.clone(),
                groups,
                average: sum as f64 / groups as f64,
                max,
            });
        }
    }

    rankings.sort();
    Ok(rankings)
}

pub fn rank_without_cache(candidates: &HashSet<String>) -> Vec<Ranking> {
    let mut rankings = Vec::new();

    for word in candidates {
        let mut pattern_counts = HashMap::new();
        for candidate in candidates {
            let pattern = diff(word, candidate);
            if pattern != 0 {
                *pattern_counts.entry(pattern).or_insert(0) += 1;
            }
        }

        let groups = pattern_counts.len();
        if groups > 0 {
            let max = *pattern_counts
                .values()
                .max()
                .expect("non-empty pattern counts");
            let sum: usize = pattern_counts.values().sum();
            rankings.push(Ranking {
                word: word.clone(),
                groups,
                average: sum as f64 / groups as f64,
                max,
            });
        }
    }

    rankings.sort();
    rankings
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{CONTAINS, HIT, Ranking, diff, rank_without_cache};

    #[test]
    fn rank_without_cache_returns_sorted_rankings() {
        let candidates = HashSet::from([
            "crate".to_string(),
            "slate".to_string(),
            "trace".to_string(),
        ]);

        assert!(rank_without_cache(&candidates).is_sorted());
    }

    #[test]
    fn ranking_order_follows_score_then_word() {
        let mut rankings = vec![
            Ranking {
                word: "max".to_string(),
                groups: 3,
                average: 4.0,
                max: 5,
            },
            Ranking {
                word: "average".to_string(),
                groups: 3,
                average: 4.0,
                max: 10,
            },
            Ranking {
                word: "groups".to_string(),
                groups: 4,
                average: 10.0,
                max: 10,
            },
            Ranking {
                word: "alpha".to_string(),
                groups: 3,
                average: 4.0,
                max: 10,
            },
        ];

        rankings.sort();

        let words: Vec<_> = rankings.iter().map(|ranking| &ranking.word).collect();
        assert_eq!(words, ["groups", "max", "alpha", "average"]);
    }

    #[test]
    fn diff_encodes_hits_and_misses() {
        assert_eq!(diff("abcde", "axcye"), 0b01_00_01_00_01);
    }

    #[test]
    fn diff_encodes_contained_letters() {
        assert_eq!(diff("abcde", "ezzzz"), CONTAINS);
        assert_eq!(diff("abcde", "abcde"), HIT * 0b01_01_01_01_01);
    }
}
