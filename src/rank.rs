use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt;

use crate::Pattern;

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

enum RankingStorage<'a> {
    All(Vec<Ranking<'a>>),
    TopK {
        rankings: BinaryHeap<Ranking<'a>>,
        k: usize,
    },
}

pub struct Rankings<'a> {
    storage: RankingStorage<'a>,
}

impl<'a> Rankings<'a> {
    pub fn new(limit: Option<usize>) -> Self {
        let storage = match limit {
            Some(limit) => RankingStorage::TopK {
                rankings: BinaryHeap::with_capacity(limit),
                k: limit,
            },
            None => RankingStorage::All(Vec::new()),
        };

        Self { storage }
    }

    pub fn push(&mut self, ranking: Ranking<'a>) {
        match &mut self.storage {
            RankingStorage::All(rankings) => rankings.push(ranking),
            RankingStorage::TopK { rankings, k } => {
                if rankings.len() < *k {
                    rankings.push(ranking);
                } else if let Some(worst) = rankings.peek()
                    && ranking.cmp(worst) == Ordering::Less
                {
                    rankings.pop();
                    rankings.push(ranking);
                }
            }
        }
    }

    pub fn into_sorted_vec(self) -> Vec<Ranking<'a>> {
        match self.storage {
            RankingStorage::All(mut rankings) => {
                rankings.sort();
                rankings
            }
            RankingStorage::TopK { rankings, .. } => rankings.into_sorted_vec(),
        }
    }
}

pub fn rank<'a>(words: &HashSet<&'a str>, limit: Option<usize>) -> Vec<Ranking<'a>> {
    let mut rankings = Rankings::new(limit);

    for guess in words {
        let mut pattern_counts = HashMap::new();
        for solution in words {
            let pattern = Pattern::new(guess, solution);
            if !pattern.is_empty() {
                *pattern_counts.entry(pattern).or_insert(0) += 1;
            }
        }

        let groups = pattern_counts.len();
        if let Some(&max) = pattern_counts.values().max() {
            let sum: usize = pattern_counts.values().sum();
            rankings.push(Ranking {
                word: guess,
                groups,
                average: sum as f64 / groups as f64,
                max,
            });
        }
    }

    rankings.into_sorted_vec()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Ranking, Rankings, rank};

    #[test]
    fn rank_without_cache_returns_sorted_rankings() {
        let candidates = HashSet::from(["crate", "slate", "trace"]);

        assert!(rank(&candidates, None).is_sorted());
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

    #[test]
    fn limited_rankings_retain_the_best_scores() {
        let mut rankings = Rankings::new(Some(2));

        for ranking in [
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
        ] {
            rankings.push(ranking);
        }

        let words: Vec<_> = rankings
            .into_sorted_vec()
            .into_iter()
            .map(|ranking| ranking.word)
            .collect();
        assert_eq!(words, ["groups", "max"]);
    }
}
