use main_error::MainError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::io::{stdout, Write};
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Serialize)]
struct History {
    path: String,
    rank: f64,
    time: u64,
}

#[derive(Clone, Copy)]
enum SortBy {
    Frecent,
    Time,
    Rank,
}

struct Args {
    add: bool,
    list: bool,
    help: bool,
    sort: SortBy,
    filter: Vec<String>,
}

impl Args {
    pub fn from_env() -> Self {
        let mut args = pico_args::Arguments::from_env();

        Args {
            add: args.contains("--add"),
            help: args.contains(["-h", "--help"]),
            sort: if args.contains("-t") {
                SortBy::Time
            } else if args.contains("-r") {
                SortBy::Rank
            } else {
                SortBy::Frecent
            },
            list: args.contains("-l"),
            filter: args
                .finish()
                .into_iter()
                .filter_map(|s| s.into_string().ok())
                .collect(),
        }
    }
}

const HOUR: u64 = 3600;
const DAY: u64 = 86_400;
const WEEK: u64 = 604_800;

impl History {
    pub fn frecent(&self, current_time: u64) -> f64 {
        match current_time - self.time {
            age if age < HOUR => self.rank * 4.0,
            age if age < DAY => self.rank * 2.0,
            age if age < WEEK => self.rank / 2.0,
            _ => self.rank / 4.0,
        }
    }

    pub fn matches(&self, pattern: &[String]) -> bool {
        let path = self.path.to_ascii_lowercase();
        let mut remaining = path.as_str();

        for pat in pattern {
            match remaining.find(pat) {
                Some(index) => remaining = &remaining[index + pat.len()..],
                None => return false,
            }
        }
        true
    }

    pub fn get_sort(&self, sort: SortBy, current_time: u64) -> f64 {
        match sort {
            SortBy::Rank => self.rank,
            SortBy::Time => self.time as f64,
            SortBy::Frecent => self.frecent(current_time),
        }
    }
}

#[test]
fn test_history_match_order() {
    assert!(History {
        path: "/foo/bar".into(),
        time: 0,
        rank: 0.0,
    }
    .matches(&["foo".into(), "bar".into()]));
    assert!(!History {
        path: "/foo/bar".into(),
        time: 0,
        rank: 0.0,
    }
    .matches(&["bar".into(), "foo".into()]));
}

#[test]
fn test_history_match_repeat() {
    assert!(History {
        path: "/foo/foo".into(),
        time: 0,
        rank: 0.0,
    }
    .matches(&["foo".into(), "foo".into()]));
    assert!(!History {
        path: "/foo/bar".into(),
        time: 0,
        rank: 0.0,
    }
    .matches(&["foo".into(), "foo".into()]));
}

fn now() -> u64 {
    let start = SystemTime::now();
    start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

fn main() -> Result<(), MainError> {
    let args = Args::from_env();

    if args.help {
        println!("zox [-h][-l][-r][-t] args");
        return Ok(());
    }

    let home = home::home_dir().expect("Cant get home directory");
    let history_path = home.join(".z");

    let history_result = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .has_headers(false)
        .from_path(history_path.clone())
        .map(|reader| reader.into_deserialize::<History>().filter_map(Result::ok));

    let now = now();

    if args.add {
        let home = home.to_str().expect("Home path not valid utf8").to_string();

        let mut history: Vec<_> = history_result
            .map(|history| history.collect())
            .unwrap_or_default();

        for path in args.filter {
            if path != home {
                let mut existing = false;
                for item in history.iter_mut() {
                    if item.path == path {
                        item.rank += 1.0;
                        item.time = now;

                        existing = true;
                    }
                }

                if !existing {
                    history.push(History {
                        path,
                        rank: 1.0,
                        time: now,
                    })
                }
            }
        }

        let total = history.iter().fold(0.0, |sum, item| sum + item.rank);

        if total > 9000.0 {
            for item in history.iter_mut() {
                item.rank *= 0.99;
            }
        }

        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'|')
            .has_headers(false)
            .from_path(history_path)?;

        for item in history.into_iter().filter(|item| item.rank >= 1.0) {
            writer.serialize(item).unwrap();
        }

        writer.flush()?;

        return Ok(());
    }

    if args.filter.is_empty() {
        let mut history: Vec<_> = history_result
            .map(|history| history.collect())
            .unwrap_or_default();

        history.sort_by(|a, b| b.rank.partial_cmp(&a.rank).unwrap());

        let stdout = stdout();
        let mut handle = stdout.lock();
        for item in history {
            let _ = writeln!(&mut handle, "{}|{}|{}", item.path, item.rank, item.time);
        }

        return Ok(());
    }

    if let Ok(history) = history_result {
        let filter: Vec<_> = args
            .filter
            .iter()
            .map(|filter| filter.to_ascii_lowercase())
            .collect();
        let matches = history.filter(|item| item.matches(&filter));

        let mut matches: Vec<History> = matches.collect();
        matches.sort_by(|a, b| {
            b.get_sort(args.sort, now)
                .partial_cmp(&a.get_sort(args.sort, now))
                .unwrap_or(Ordering::Equal)
        });

        if args.list {
            let out = stdout();
            let mut out = out.lock();
            for item in matches {
                writeln!(
                    &mut out,
                    "{:<11}{}",
                    item.get_sort(args.sort, now),
                    item.path
                )
                .ok();
            }
        } else {
            if let Some(first) = matches.first() {
                println!("{}", first.path);
            } else {
                exit(1);
            }
        }
    }

    Ok(())
}
