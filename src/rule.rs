#[derive(Debug)]
pub enum Rule {
    Contains(char, u8),
    Match(char, u8),
    None(char),
    Once(char),
}

impl Rule {
    pub fn matches(&self, word: &str) -> bool {
        match *self {
            Rule::Match(ch, pos) => word.chars().nth((pos - 1) as usize) == Some(ch),
            Rule::Contains(ch, pos) => {
                word.contains(ch) && !(word.chars().nth((pos - 1) as usize) == Some(ch))
            }
            Rule::None(ch) => !word.contains(ch),
            Rule::Once(ch) => word.chars().filter(|&c| c == ch).count() == 1,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CharPos {
    ch: char,
    pos: u8,
}

impl TryFrom<&str> for CharPos {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut chars = value.chars();
        let ch = chars
            .next()
            .ok_or_else(|| format!("Invalid entry '{}'", value))?
            .to_ascii_lowercase();
        let pos = chars
            .as_str()
            .parse()
            .map_err(|_| format!("Invalid number in '{}'", value))?;

        Ok(Self { ch, pos })
    }
}

pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn builder() -> RuleSetBuilder {
        RuleSetBuilder::default()
    }

    pub fn push(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn matches(&self, word: &str) -> bool {
        self.rules.iter().all(|rule| rule.matches(word))
    }

    pub fn filter<'a>(&self, words: &[&'a str]) -> HashSet<&'a str> {
        words
            .iter()
            .filter(|word| self.matches(word))
            .copied()
            .collect()
    }
}

#[derive(Default)]
pub struct RuleSetBuilder {
    match_values: Vec<String>,
    contains_values: Vec<String>,
    none_values: Vec<String>,
    once_values: Vec<String>,
}

impl RuleSetBuilder {
    pub fn matches(mut self, values: Vec<String>) -> Self {
        self.match_values = values;
        self
    }

    pub fn contains(mut self, values: Vec<String>) -> Self {
        self.contains_values = values;
        self
    }

    pub fn none(mut self, values: Vec<String>) -> Self {
        self.none_values = values;
        self
    }

    pub fn once(mut self, values: Vec<String>) -> Self {
        self.once_values = values;
        self
    }

    pub fn build(self) -> Result<RuleSet, String> {
        let mut rules = RuleSet::new();

        for value in self.match_values {
            for CharPos { ch, pos } in value
                .split(',')
                .map(CharPos::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Error parsing match: {error}"))?
            {
                rules = rules.push(Rule::Match(ch, pos));
            }
        }

        for value in self.contains_values {
            for CharPos { ch, pos } in value
                .split(',')
                .map(CharPos::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Error parsing contains: {error}"))?
            {
                rules = rules.push(Rule::Contains(ch, pos));
            }
        }

        for value in self.none_values {
            for ch in value
                .split(',')
                .filter_map(|s| s.chars().next().map(|ch| ch.to_ascii_lowercase()))
            {
                rules = rules.push(Rule::None(ch));
            }
        }

        for value in self.once_values {
            for ch in value
                .split(',')
                .filter_map(|s| s.chars().next().map(|ch| ch.to_ascii_lowercase()))
            {
                rules = rules.push(Rule::Once(ch));
            }
        }

        Ok(rules)
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{CharPos, Rule, RuleSet};

    #[test]
    fn empty_rulesets_match_every_word() {
        assert!(RuleSet::new().matches("slate"));
        assert!(RuleSet::default().matches("slate"));
    }

    #[test]
    fn ruleset_pushes_single_rules() {
        let rules = RuleSet::new()
            .push(Rule::Match('s', 1))
            .push(Rule::Contains('l', 3))
            .push(Rule::None('r'))
            .push(Rule::None('n'))
            .push(Rule::Once('e'));

        assert!(rules.matches("slate"));
        assert!(!rules.matches("spate"));
    }

    #[test]
    fn builder_creates_ruleset() {
        let rules = RuleSet::builder()
            .matches(vec!["S1".to_string()])
            .contains(vec!["L3".to_string()])
            .none(vec!["R,N".to_string()])
            .once(vec!["E".to_string()])
            .build()
            .unwrap();

        assert!(rules.matches("slate"));
        assert!(!rules.matches("spate"));
    }

    #[test]
    fn char_pos_normalizes_uppercase_letters() {
        assert_eq!(CharPos::try_from("S1"), Ok(CharPos { ch: 's', pos: 1 }));
    }

    #[test]
    fn filter_returns_words_matching_rules() {
        let words = vec!["slate", "crate", "caper"];
        let rules = RuleSet::new().push(Rule::Match('s', 1));

        assert_eq!(rules.filter(&words), HashSet::from(["slate"]));
    }
}
use std::collections::HashSet;
