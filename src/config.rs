/// Code for reading the event handler config file into memory.
use ini::Ini;
use std::error::Error;
use std::path::{Path, PathBuf};

fn parse_yesno(val: &String) -> bool {
    val == "yes"
}

/// All of the runtime configuration for the event handler.
#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    pub db_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            db_path: PathBuf::from("events.db"),
        }
    }
}

impl Config {
    /// Reads the runtime configuration from an INI config file at the given path, applying defaults for individual
    /// configuration values that aren't present, or returning an error if no readable configuration is present at all.
    pub fn from_path<P>(path: P) -> Result<Self, Box<Error>>
    where
        P: AsRef<Path>,
    {
        let conf = Ini::load_from_file(path)?;
        let section = conf.section(Some("general"))
            .ok_or("No 'general' section present in the config file.")?;
        let defaults: Config = Default::default();
        Ok(Self {
            enabled: section.get("enabled").map(parse_yesno).unwrap_or(defaults.enabled),
            db_path: section.get("db_path").map(PathBuf::from).unwrap_or(defaults.db_path),
        })
    }
}
