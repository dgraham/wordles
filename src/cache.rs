use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::env;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::{Utf8Error, from_utf8};

use lmdb::{Database, DatabaseFlags, Environment, Transaction, WriteFlags};

use crate::{Ranking, diff};

const MAP_SIZE: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum CacheError {
    Lmdb(lmdb::Error),
    Utf8(Utf8Error),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lmdb(error) => error.fmt(f),
            Self::Utf8(error) => error.fmt(f),
        }
    }
}

impl Error for CacheError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Lmdb(error) => Some(error),
            Self::Utf8(error) => Some(error),
        }
    }
}

impl From<lmdb::Error> for CacheError {
    fn from(error: lmdb::Error) -> Self {
        Self::Lmdb(error)
    }
}

impl From<Utf8Error> for CacheError {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

pub struct Cache {
    env: Environment,
    db: Database,
}

impl Cache {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, lmdb::Error> {
        let env = Environment::new()
            .set_map_size(MAP_SIZE)
            .open(path.as_ref())?;
        let db = env.create_db(None, DatabaseFlags::empty())?;

        Ok(Self { env, db })
    }

    pub fn exists(path: impl AsRef<Path>) -> bool {
        path.as_ref().join("data.mdb").is_file()
    }

    pub fn write(&self, words: &[String]) -> Result<(), lmdb::Error> {
        let mut txn = self.env.begin_rw_txn()?;
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
            txn.put(self.db, word, &pattern_keys, WriteFlags::empty())?;

            for pattern in pattern_order {
                let candidates = patterns
                    .get(&pattern)
                    .expect("pattern order only contains inserted patterns");
                let key = format!("{word}:{pattern}");
                let value = candidates.join(",");
                txn.put(self.db, &key, &value, WriteFlags::empty())?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    pub fn default_path() -> Result<PathBuf, io::Error> {
        let cache_home = env::var_os("XDG_CACHE_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .filter(|path| !path.is_empty())
                    .map(|home| PathBuf::from(home).join(".cache"))
            })
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "XDG_CACHE_HOME and HOME are unset")
            })?;

        Ok(cache_home.join("wordles"))
    }

    pub fn rank(&self, candidates: &HashSet<String>) -> Result<Vec<Ranking>, CacheError> {
        let txn = self.env.begin_ro_txn()?;
        let mut rankings = Vec::new();

        for word in candidates {
            let patterns = from_utf8(txn.get(self.db, word)?)?;
            let mut groups = 0;
            let mut max = 0;
            let mut sum = 0;

            for pattern in patterns.split(',').filter(|pattern| !pattern.is_empty()) {
                let key = format!("{word}:{pattern}");
                let words = from_utf8(txn.get(self.db, &key)?)?;
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
                rankings.push(Ranking {
                    word: word.clone(),
                    groups,
                    average: sum as f64 / groups as f64,
                    max,
                });
            }
        }

        rankings.sort();
        Ok(rankings)
    }
}
