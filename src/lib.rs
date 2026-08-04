use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;

pub mod cache;
pub mod freq;
pub mod pattern;
pub mod rule;

pub use pattern::Pattern;

pub struct Ranking<'a> {
    pub word: &'a str,
    pub groups: usize,
    pub average: f64,
    pub max: usize,
}

impl<'a> Ord for Ranking<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .groups
            .cmp(&self.groups)
            .then_with(|| self.average.total_cmp(&other.average))
            .then_with(|| self.max.cmp(&other.max))
            .then_with(|| self.word.cmp(other.word))
    }
}

impl<'a> PartialOrd for Ranking<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> PartialEq for Ranking<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'a> Eq for Ranking<'a> {}

impl<'a> fmt::Display for Ranking<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.word, self.groups, self.max, self.average
        )
    }
}

pub fn rank<'a>(candidates: &HashSet<&'a str>) -> Vec<Ranking<'a>> {
    let mut rankings = Vec::new();

    for word in candidates {
        let mut pattern_counts = HashMap::new();
        for candidate in candidates {
            let pattern = Pattern::new(word, candidate);
            if !pattern.is_empty() {
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
                word,
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

    use super::{Ranking, rank};

    #[test]
    fn rank_without_cache_returns_sorted_rankings() {
        let candidates = HashSet::from([
            "crate",
            "slate",
            "trace",
        ]);

        assert!(rank(&candidates).is_sorted());
    }

    #[test]
    fn ranking_order_follows_score_then_word() {
        let mut rankings = vec![
            Ranking {
                word: "max",
                groups: 3,
                average: 4.0,
                max: 5,
            },
            Ranking {
                word: "average",
                groups: 3,
                average: 4.0,
                max: 10,
            },
            Ranking {
                word: "groups",
                groups: 4,
                average: 10.0,
                max: 10,
            },
            Ranking {
                word: "alpha",
                groups: 3,
                average: 4.0,
                max: 10,
            },
        ];

        rankings.sort();

        let words: Vec<_> = rankings.iter().map(|ranking| ranking.word).collect();
        assert_eq!(words, &["groups", "max", "alpha", "average"]);
    }

    #[test]
    fn ranking_displays_scores() {
        let ranking = Ranking {
            word: "slate",
            groups: 12,
            average: 1.5,
            max: 3,
        };

        assert_eq!(ranking.to_string(), "slate 12 3 1.5");
    }

}
