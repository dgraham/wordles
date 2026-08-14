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
        RuleSetBuilder::new()
    }

    pub fn push(&mut self, rule: Rule) {
        self.rules.push(rule);
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

impl Extend<Rule> for RuleSet {
    fn extend<T: IntoIterator<Item = Rule>>(&mut self, rules: T) {
        self.rules.extend(rules);
    }
}

impl FromIterator<Rule> for RuleSet {
    fn from_iter<T: IntoIterator<Item = Rule>>(rules: T) -> Self {
        let mut set = Self::new();
        set.extend(rules);
        set
    }
}

pub struct RuleSetBuilder {
    result: Result<RuleSet, String>,
}

impl RuleSetBuilder {
    pub fn new() -> Self {
        Self {
            result: Ok(RuleSet::new()),
        }
    }

    pub fn matches(self, values: &[impl AsRef<str>]) -> Self {
        self.position_rules(values, Rule::Match)
    }

    pub fn contains(self, values: &[impl AsRef<str>]) -> Self {
        self.position_rules(values, Rule::Contains)
    }

    pub fn none(self, values: &[impl AsRef<str>]) -> Self {
        self.character_rules(values, Rule::None)
    }

    pub fn once(self, values: &[impl AsRef<str>]) -> Self {
        self.character_rules(values, Rule::Once)
    }

    pub fn build(self) -> Result<RuleSet, String> {
        self.result
    }

    fn position_rules(mut self, values: &[impl AsRef<str>], rule: fn(char, u8) -> Rule) -> Self {
        self.result = self.result.and_then(|mut rules| {
            for value in values {
                let positions = value
                    .as_ref()
                    .split(',')
                    .map(CharPos::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| format!("Error parsing rule: {error}"))?;
                rules.extend(
                    positions
                        .into_iter()
                        .map(|CharPos { ch, pos }| rule(ch, pos)),
                );
            }
            Ok(rules)
        });

        self
    }

    fn character_rules(mut self, values: &[impl AsRef<str>], rule: fn(char) -> Rule) -> Self {
        self.result = self.result.map(|mut rules| {
            for value in values {
                rules.extend(
                    value
                        .as_ref()
                        .split(',')
                        .filter_map(|value| value.chars().next())
                        .map(|ch| rule(ch.to_ascii_lowercase())),
                );
            }
            rules
        });

        self
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RuleSetBuilder {
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
        let mut rules = RuleSet::new();
        rules.push(Rule::Match('s', 1));
        rules.push(Rule::Contains('l', 3));
        rules.push(Rule::None('r'));
        rules.push(Rule::None('n'));
        rules.push(Rule::Once('e'));

        assert!(rules.matches("slate"));
        assert!(!rules.matches("spate"));
    }

    #[test]
    fn builder_creates_ruleset() {
        let rules = RuleSet::builder()
            .matches(&["S1"])
            .contains(&["L3"])
            .none(&["R,N"])
            .once(&["E"])
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
        let rules: RuleSet = [Rule::Match('s', 1)].into_iter().collect();

        assert_eq!(rules.filter(&words), HashSet::from(["slate"]));
    }
}
use std::collections::HashSet;
