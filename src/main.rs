use std::borrow::Cow;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs::{create_dir_all, read_to_string};
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use getopts::Options;
use wordles::{Cache, CharFreq, Pattern, Ranking, RuleSet, rank};

const WORDS: &str = include_str!("../data/words");

fn main() -> Result<(), Box<dyn Error>> {
    let args = args()?;
    let opts = opts();
    let matches = opts.parse(&args[1..])?;

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
        print!("{WORDS}");
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
                    format!("Invalid --limit value: {value}"),
                )
            })?,
            None => 5,
        })
    };

    let dictionary = match matches.opt_str("dict") {
        Some(path) => Cow::Owned(read_to_string(path)?.to_lowercase()),
        None => Cow::Borrowed(WORDS),
    };
    let words: Vec<&str> = dictionary
        .lines()
        .map(|word| {
            if word.chars().count() == 5 {
                Ok(word)
            } else {
                Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("Dictionary word must have five characters: {word}"),
                ))
            }
        })
        .collect::<Result<_, _>>()?;

    if matches.opt_present("frequency") {
        for frequency in CharFreq::frequencies(&words) {
            println!("{} {}", frequency.ch, frequency.percentage * 100.0);
        }
        return Ok(());
    }

    let rules = RuleSet::builder()
        .matches(&matches.opt_strs("match"))
        .contains(&matches.opt_strs("contains"))
        .none(&matches.opt_strs("none"))
        .once(&matches.opt_strs("once"))
        .build()?;

    let candidates = rules.filter(&words);

    let rankings = if matches.opt_present("no-cache") {
        rank(&candidates, limit)
    } else {
        let path = matches
            .opt_str("cache")
            .map(PathBuf::from)
            .unwrap_or(Cache::default_path()?);
        let needs_write = !Cache::exists(&path);
        create_dir_all(&path)?;
        let cache = Cache::open(path)?;
        if needs_write {
            cache.write(&words)?;
        }
        cache.rank(&candidates, limit)?
    };

    if matches.opt_present("verbose") {
        print_verbose(&rankings);
    } else {
        print_inline(&rankings);
    }

    Ok(())
}

fn opts() -> Options {
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
    opts
}

fn args() -> Result<Vec<String>, io::Error> {
    env::args_os()
        .map(|arg| {
            arg.into_string().map_err(|arg| {
                io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid UTF-8 argument: {}", arg.to_string_lossy()),
                )
            })
        })
        .collect()
}

fn print_patterns() {
    let words = ["crest", "slate", "audio", "train", "heist", "adore"];

    for solution in words {
        let mut patterns: BTreeMap<Pattern, Vec<&str>> = BTreeMap::new();
        for guess in words {
            let pattern = Pattern::new(guess, solution);
            if !pattern.is_empty() {
                patterns.entry(pattern).or_default().push(guess);
            }
        }

        println!("{solution}");
        for (pattern, guesses) in patterns {
            let highlights = guesses
                .iter()
                .map(|word| pattern.highlight(word))
                .collect::<Vec<_>>()
                .join(", ");
            println!("{} [{highlights}]", pattern.emoji());
        }
        println!();
    }
}

fn print_verbose(rankings: &[Ranking]) {
    let output = rankings
        .iter()
        .rev()
        .map(Ranking::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    if !output.is_empty() {
        println!("{output}");
    }
}

fn print_inline(rankings: &[Ranking]) {
    println!(
        "{}",
        rankings
            .iter()
            .map(|ranking| ranking.word)
            .collect::<Vec<_>>()
            .join(" ")
    );
}
