use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::env;
use std::error::Error;
use std::fs::{create_dir_all, read_to_string};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use getopts::Options;
use lmdb::{Database, DatabaseFlags, Environment, Transaction, WriteFlags};

const MAP_SIZE: usize = 64 * 1024 * 1024;
const SOLUTIONS: &str = include_str!("../data/words");
const MISS: u16 = 0b00;
const HIT: u16 = 0b01;
const CONTAINS: u16 = 0b10;

#[derive(Debug)]
enum Rule {
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
            Rule::Once(ch) => {
                let count = word.chars().filter(|&c| c == ch).count();
                count == 1
            }
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
                .ok_or_else(|| format!("Invalid entry '{}'", item))?;
            let num: u8 = rest
                .parse()
                .map_err(|_| format!("Invalid number in '{}'", item))?;
            Ok((ch, num))
        })
        .collect()
}

fn read_words(dictionary: &str) -> Vec<String> {
    dictionary.lines().map(str::to_lowercase).collect()
}

fn print_frequency(words: &[String]) {
    let mut counts: Vec<(char, usize)> = Vec::new();

    for word in words {
        for ch in word.chars() {
            if let Some((_, count)) = counts.iter_mut().find(|(letter, _)| *letter == ch) {
                *count += 1;
            } else {
                counts.push((ch, 1));
            }
        }
    }

    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let total = (words.len() * 5) as f64;
    for (ch, count) in counts {
        println!("{ch} {}", count as f64 / total * 100.0);
    }
}

fn diff(guess: &str, candidate: &str) -> u16 {
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

fn square(value: u16) -> &'static str {
    match value {
        MISS => "⬜️",
        HIT => "🟩",
        CONTAINS => "🟨",
        _ => "⬜️",
    }
}

fn emoji(pattern: u16) -> String {
    [8, 6, 4, 2, 0]
        .iter()
        .map(|shift| square((pattern >> shift) & 0b11))
        .collect::<Vec<_>>()
        .join(" ")
}

fn colored_word(word: &str, pattern: u16) -> String {
    word.chars()
        .enumerate()
        .map(|(index, ch)| {
            let value = (pattern >> (8 - index * 2)) & 0b11;
            let color = match value {
                MISS => "\x1b[100;37;1m",
                HIT => "\x1b[42;30;1m",
                CONTAINS => "\x1b[43;30;1m",
                _ => "\x1b[100;37;1m",
            };
            format!("{color} {ch} \x1b[0m")
        })
        .collect()
}

fn print_patterns() {
    let words = ["crest", "slate", "audio", "train", "heist", "adore"];

    for solution in words {
        let mut patterns: HashMap<u16, Vec<&str>> = HashMap::new();
        let mut pattern_order = Vec::new();

        for candidate in words {
            let pattern = diff(candidate, solution);
            if pattern != 0 {
                match patterns.entry(pattern) {
                    Entry::Occupied(entry) => entry.into_mut().push(candidate),
                    Entry::Vacant(entry) => {
                        pattern_order.push(pattern);
                        entry.insert(vec![candidate]);
                    }
                }
            }
        }

        println!("{solution}");
        for pattern in pattern_order {
            let candidates = patterns
                .get(&pattern)
                .expect("pattern order only contains inserted patterns");
            let words = candidates
                .iter()
                .map(|candidate| colored_word(candidate, pattern))
                .collect::<Vec<_>>()
                .join(", ");
            println!("{} [{words}]", emoji(pattern));
        }
        println!();
    }
}

fn dump_patterns(words: &[String], path: &Path) -> Result<(), Box<dyn Error>> {
    create_dir_all(path)?;

    let env = Environment::new().set_map_size(MAP_SIZE).open(path)?;
    let db = env.create_db(None, DatabaseFlags::empty())?;
    let mut txn = env.begin_rw_txn()?;
    for word in words {
        let mut patterns: HashMap<u16, Vec<&str>> = HashMap::new();
        let mut pattern_order = Vec::new();

        for candidate in words {
            let pattern = diff(word, candidate);
            if pattern != 0 {
                match patterns.entry(pattern) {
                    Entry::Occupied(entry) => entry.into_mut().push(candidate),
                    Entry::Vacant(entry) => {
                        pattern_order.push(pattern);
                        entry.insert(vec![candidate]);
                    }
                }
            }
        }

        let pattern_keys = pattern_order
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        txn.put(db, word, &pattern_keys, WriteFlags::empty())?;

        for pattern in pattern_order {
            let candidates = patterns
                .get(&pattern)
                .expect("pattern order only contains inserted patterns");
            let key = format!("{word}:{pattern}");
            let value = candidates.join(",");
            txn.put(db, &key, &value, WriteFlags::empty())?;
        }
    }

    txn.commit()?;
    Ok(())
}

fn default_cache_path() -> Result<PathBuf, io::Error> {
    let cache_home = env::var_os("XDG_CACHE_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|path| !path.is_empty())
                .map(|home| PathBuf::from(home).join(".cache"))
        })
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "XDG_CACHE_HOME and HOME are unset"))?;

    Ok(cache_home.join("wordles"))
}

type Ranking = (String, usize, f64, usize);

fn rank_from_cache<T: Transaction>(
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

        for pattern in patterns.split(',') {
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
            rankings.push((word.clone(), groups, sum as f64 / groups as f64, max));
        }
    }

    Ok(rankings)
}

fn rank_without_cache(candidates: &HashSet<String>) -> Vec<Ranking> {
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
            rankings.push((word.clone(), groups, sum as f64 / groups as f64, max));
        }
    }

    rankings
}

fn print_rankings(mut rankings: Vec<Ranking>, verbose: bool) {
    rankings.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.0.cmp(&b.0))
    });

    let rankings: Vec<_> = rankings.into_iter().take(10).collect();
    if verbose {
        for (word, count, avg, max) in rankings.into_iter().rev() {
            println!("{word} {count} {max} {avg}");
        }
    } else {
        println!(
            "{}",
            rankings
                .iter()
                .map(|(word, _, _, _)| word.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    opts.optmulti(
        "c",
        "contains",
        "Comma-delimited list of <char><pos> for contains rules",
        "L3",
    );
    opts.optmulti(
        "m",
        "match",
        "Comma-delimited list of <char><pos> for match rules",
        "S1,E5",
    );
    opts.optmulti(
        "n",
        "none",
        "Comma-delimited list of <char> for none rules",
        "R,N",
    );
    opts.optmulti(
        "o",
        "once",
        "Comma-delimited list of <char> for once rules",
        "E",
    );
    opts.optflag(
        "",
        "no-cache",
        "Rank without reading or writing the LMDB cache",
    );
    opts.optopt(
        "",
        "cache",
        "Read or create the LMDB cache at this path",
        "DIR",
    );
    opts.optopt("", "dict", "Read words from dictionary file", "FILE");
    opts.optflag("", "frequency", "Print character frequencies");
    opts.optflag("", "patterns", "Print sample patterns");
    opts.optflag("", "words", "Print built-in dictionary");
    opts.optflag("v", "verbose", "Print ranked words with scores");
    opts.optflag("V", "version", "Print version information");
    opts.optflag("h", "help", "Print this help message");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            std::process::exit(1);
        }
    };

    if matches.opt_present("help") {
        println!("A Wordle solver\n");
        print!("{}", opts.usage(&format!("Usage: {} [options]", args[0])));
        return Ok(());
    }
    if matches.opt_present("version") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if matches.opt_present("patterns") {
        print_patterns();
        return Ok(());
    }
    if matches.opt_present("words") {
        print!("{SOLUTIONS}");
        return Ok(());
    }
    if matches.opt_present("no-cache") && matches.opt_present("cache") {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "--cache cannot be used with --no-cache",
        )
        .into());
    }

    let words = match matches.opt_str("dict") {
        Some(path) => read_words(&read_to_string(path)?),
        None => read_words(SOLUTIONS),
    };
    if matches.opt_present("frequency") {
        print_frequency(&words);
        return Ok(());
    }

    let mut rules = Vec::new();

    for m in matches.opt_strs("match") {
        match split_char_pos(&m) {
            Ok(v) => {
                v.iter().for_each(|&(ch, num)| {
                    rules.push(Rule::Match(ch, num));
                });
            }
            Err(e) => {
                eprintln!("Error parsing match: {}", e);
                std::process::exit(1);
            }
        }
    }
    for m in matches.opt_strs("contains") {
        match split_char_pos(&m) {
            Ok(v) => {
                v.iter().for_each(|&(ch, num)| {
                    rules.push(Rule::Contains(ch, num));
                });
            }
            Err(e) => {
                eprintln!("Error parsing contains: {}", e);
                std::process::exit(1);
            }
        }
    }
    for m in matches.opt_strs("none") {
        let none: Vec<Rule> = m
            .split(',')
            .filter_map(|s| s.chars().next())
            .map(Rule::None)
            .collect();
        rules.extend(none);
    }
    for m in matches.opt_strs("once") {
        let once: Vec<Rule> = m
            .split(',')
            .filter_map(|s| s.chars().next())
            .map(Rule::Once)
            .collect();
        rules.extend(once);
    }

    let candidates: HashSet<String> = words
        .iter()
        .filter(|word| rules.iter().all(|rule| rule.matches(word)))
        .cloned()
        .collect();

    let rankings = if matches.opt_present("no-cache") {
        rank_without_cache(&candidates)
    } else {
        let path = matches
            .opt_str("cache")
            .map(PathBuf::from)
            .unwrap_or(default_cache_path()?);
        if !path.join("data.mdb").is_file() {
            dump_patterns(&words, &path)?;
        }

        let env = Environment::new().set_map_size(MAP_SIZE).open(&path)?;
        let db = env.open_db(None)?;
        let txn = env.begin_ro_txn()?;
        rank_from_cache(&txn, db, &candidates)?
    };
    print_rankings(rankings, matches.opt_present("verbose"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CONTAINS, HIT, diff};

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
