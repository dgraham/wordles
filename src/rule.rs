use getopts::Matches;

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
            Rule::Match(ch, pos) => word
                .chars()
                .nth((pos - 1) as usize)
                .map_or(false, |c| c == ch),
            Rule::Contains(ch, pos) => {
                word.contains(ch)
                    && !word
                        .chars()
                        .nth((pos - 1) as usize)
                        .map_or(false, |c| c == ch)
            }
            Rule::None(ch) => !word.contains(ch),
            Rule::Once(ch) => word.chars().filter(|&c| c == ch).count() == 1,
        }
    }
}

fn split_char_pos(s: &str) -> Result<Vec<(char, u8)>, String> {
    s.split(',')
        .map(|item| {
            let (ch, rest) = item.split_at(1);
            let ch = ch
                .chars()
                .next()
                .ok_or_else(|| format!("Invalid entry '{}'", item))?
                .to_ascii_lowercase();
            let pos: u8 = rest
                .parse()
                .map_err(|_| format!("Invalid number in '{}'", item))?;
            Ok((ch, pos))
        })
        .collect()
}

pub fn rules_from_matches(matches: &Matches) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();

    for value in matches.opt_strs("match") {
        let rules_from_value = split_char_pos(&value)
            .map_err(|error| format!("Error parsing match: {error}"))?
            .into_iter()
            .map(|(ch, pos)| Rule::Match(ch, pos));
        rules.extend(rules_from_value);
    }

    for value in matches.opt_strs("contains") {
        let rules_from_value = split_char_pos(&value)
            .map_err(|error| format!("Error parsing contains: {error}"))?
            .into_iter()
            .map(|(ch, pos)| Rule::Contains(ch, pos));
        rules.extend(rules_from_value);
    }

    for value in matches.opt_strs("none") {
        let rules_from_value = value
            .split(',')
            .filter_map(|s| s.chars().next().map(|ch| ch.to_ascii_lowercase()))
            .map(Rule::None);
        rules.extend(rules_from_value);
    }

    for value in matches.opt_strs("once") {
        let rules_from_value = value
            .split(',')
            .filter_map(|s| s.chars().next().map(|ch| ch.to_ascii_lowercase()))
            .map(Rule::Once);
        rules.extend(rules_from_value);
    }

    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::split_char_pos;

    #[test]
    fn split_char_pos_normalizes_uppercase_letters() {
        assert_eq!(split_char_pos("S1,L2"), Ok(vec![('s', 1), ('l', 2)]));
    }
}
