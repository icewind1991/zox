use main_error::MainError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
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
            filter: args.free().unwrap_or_default(),
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
        pattern.iter().all(|pat| self.path.contains(pat))
    }

    pub fn get_sort(&self, sort: SortBy, current_time: u64) -> f64 {
        match sort {
            SortBy::Rank => self.rank,
            SortBy::Time => self.time as f64,
            SortBy::Frecent => self.frecent(current_time),
        }
    }
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

    let home = home::home_dir().expect("Cant get home directory");
    let history_path = home.join(".z");

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .has_headers(false)
        .from_path(history_path.clone())
        .unwrap();

    let now = now();

    if args.help {
        println!("zox [-h][-l][-r][-t] args");
        return Ok(());
    }

    let history = reader
        .deserialize::<History>()
        .filter_map(|result| result.ok());

    if args.add {
        let home = home.to_str().expect("Home path not valid utf8").to_string();

        let mut history: Vec<_> = history.collect();

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

    let matches = history.filter(|item| item.matches(&args.filter));

    if args.list {
        for item in matches {
            println!("{:<11}{}", item.get_sort(args.sort, now), item.path);
        }
    } else {
        let mut matches: Vec<History> = matches.collect();
        matches.sort_by(
            |a, b| match a.get_sort(args.sort, now) - b.get_sort(args.sort, now) {
                diff if diff < 0.0 => Ordering::Greater,
                diff if diff > 0.0 => Ordering::Less,
                _ => Ordering::Equal,
            },
        );

        if let Some(first) = matches.first() {
            println!("{}", first.path);
        }
    }

    Ok(())
}
