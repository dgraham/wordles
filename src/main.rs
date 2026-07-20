use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::env;
use std::error::Error;
use std::fs::{create_dir_all, read_to_string};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use getopts::Options;
use lmdb::{DatabaseFlags, Environment, Transaction, WriteFlags};
use wordles::rule::RuleSet;
use wordles::{CONTAINS, HIT, MISS, Ranking, diff, rank_from_cache, rank_without_cache};

const MAP_SIZE: usize = 64 * 1024 * 1024;
const SOLUTIONS: &str = include_str!("../data/words");

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

fn print_rankings(rankings: Vec<Ranking>, limit: Option<usize>, verbose: bool) {
    let rankings = match limit {
        Some(limit) => rankings.into_iter().take(limit).collect(),
        None => rankings,
    };

    if verbose {
        for ranking in rankings.into_iter().rev() {
            println!(
                "{} {} {} {}",
                ranking.word, ranking.groups, ranking.max, ranking.average
            );
        }
    } else {
        println!(
            "{}",
            rankings
                .iter()
                .map(|ranking| ranking.word.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::new();
    opts.optmulti("c", "contains", "List of contains rules", "L3");
    opts.optmulti("m", "match", "List of match rules", "S1,E5");
    opts.optmulti("n", "none", "List of none rules", "R,N");
    opts.optmulti("o", "once", "List of once rules", "E");
    opts.optflag("", "no-cache", "Rank without reading or writing the cache");
    opts.optopt("", "cache", "Read or create the cache at this path", "DIR");
    opts.optopt("", "dict", "Read words from dictionary file", "FILE");
    opts.optopt("", "limit", "Limit output words (default: 5)", "NUM");
    opts.optflag("", "no-limit", "Print all candidate words");
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

    if matches.opt_present("limit") && matches.opt_present("no-limit") {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "--limit cannot be used with --no-limit",
        )
        .into());
    }

    let limit = if matches.opt_present("no-limit") {
        None
    } else {
        Some(match matches.opt_str("limit") {
            Some(value) => value.parse::<usize>().map_err(|_| {
                io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("invalid --limit value: {value}"),
                )
            })?,
            None => 5,
        })
    };

    let words = match matches.opt_str("dict") {
        Some(path) => read_words(&read_to_string(path)?),
        None => read_words(SOLUTIONS),
    };

    if matches.opt_present("frequency") {
        print_frequency(&words);
        return Ok(());
    }

    let rules = match RuleSet::builder()
        .matches(matches.opt_strs("match"))
        .contains(matches.opt_strs("contains"))
        .none(matches.opt_strs("none"))
        .once(matches.opt_strs("once"))
        .build()
    {
        Ok(rules) => rules,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let candidates: HashSet<String> = words
        .iter()
        .filter(|word| rules.matches(word))
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
    print_rankings(rankings, limit, matches.opt_present("verbose"));

    Ok(())
}
