use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct RuleError {
    message: String,
}

impl RuleError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for RuleError {}

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
    type Error = RuleError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut chars = value.chars();
        let ch = chars
            .next()
            .ok_or_else(|| RuleError::new(format!("Invalid entry '{value}'")))?
            .to_ascii_lowercase();
        let pos = chars
            .as_str()
            .parse()
            .map_err(|_| RuleError::new(format!("Invalid number in '{value}'")))?;
        if !(1..=5).contains(&pos) {
            return Err(RuleError::new(format!("Invalid position in '{value}'")));
        }

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
    result: Result<RuleSet, RuleError>,
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

    pub fn build(self) -> Result<RuleSet, RuleError> {
        self.result
    }

    fn position_rules(mut self, values: &[impl AsRef<str>], rule: fn(char, u8) -> Rule) -> Self {
        self.result = self.result.and_then(|mut rules| {
            for value in values {
                let positions = value
                    .as_ref()
                    .split(',')
                    .map(|value| CharPos::try_from(value).map(|CharPos { ch, pos }| rule(ch, pos)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| RuleError::new(format!("Error parsing rule: {error}")))?;
                rules.extend(positions);
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

    use super::{CharPos, Rule, RuleError, RuleSet};

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
    fn char_pos_rejects_positions_outside_word_bounds() {
        assert_eq!(
            CharPos::try_from("S0"),
            Err(RuleError::new("Invalid position in 'S0'"))
        );
        assert_eq!(
            CharPos::try_from("S6"),
            Err(RuleError::new("Invalid position in 'S6'"))
        );
    }

    #[test]
    fn filter_returns_words_matching_rules() {
        let words = vec!["slate", "crate", "caper"];
        let rules: RuleSet = [Rule::Match('s', 1)].into_iter().collect();

        assert_eq!(rules.filter(&words), HashSet::from(["slate"]));
    }
}
use std::collections::HashSet;
