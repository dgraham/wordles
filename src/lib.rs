use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;

pub mod cache;
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

impl fmt::Display for Ranking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.word, self.groups, self.max, self.average
        )
    }
}

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

pub fn rank(candidates: &HashSet<String>) -> Vec<Ranking> {
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

    use super::{CONTAINS, HIT, Ranking, diff, rank};

    #[test]
    fn rank_without_cache_returns_sorted_rankings() {
        let candidates = HashSet::from([
            "crate".to_string(),
            "slate".to_string(),
            "trace".to_string(),
        ]);

        assert!(rank(&candidates).is_sorted());
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
    fn ranking_displays_scores() {
        let ranking = Ranking {
            word: "slate".to_string(),
            groups: 12,
            average: 1.5,
            max: 3,
        };

        assert_eq!(ranking.to_string(), "slate 12 3 1.5");
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
