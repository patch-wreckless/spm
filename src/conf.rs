use std::{
    env,
    fmt::{self, Display},
    path::PathBuf,
};

use serde::Deserialize;
use toml::value::{Table as TomlTable, Value as TomlValue};

#[derive(Debug)]
pub enum ConfigError {
    ReadError(std::io::Error),
    ParseError(toml::de::Error),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ReadError(e) => write!(f, "failed to read config file: {}", e),
            ConfigError::ParseError(e) => write!(f, "failed to parse config file: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::ReadError(e) => Some(e),
            ConfigError::ParseError(e) => Some(e),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub registry: Registry,
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub url: String,
}

pub fn load_config(overrides: &[(String, String)]) -> Result<Config, ConfigError> {
    let mut merged = load_config_from_file()?;
    apply_overrides(&mut merged, load_config_from_env())?;
    let explicit_overrides = overrides.iter().map(|(key, val)| {
        let path: Vec<String> = key.split('.').map(|s| s.to_string()).collect();
        (path, val.clone())
    });
    apply_overrides(&mut merged, explicit_overrides)?;
    let config: Config = merged.try_into().map_err(ConfigError::ParseError)?;
    Ok(config)
}

fn load_config_from_file() -> Result<TomlTable, ConfigError> {
    match read_config_file() {
        None => Ok(TomlTable::new()),
        Some(res) => match res {
            Err(e) => Err(ConfigError::ReadError(e)),
            Ok(contents) => {
                let toml_value: TomlValue =
                    toml::from_slice(&contents).map_err(ConfigError::ParseError)?;
                match toml_value {
                    TomlValue::Table(t) => Ok(t),
                    _ => Ok(TomlTable::new()),
                }
            }
        },
    }
}

fn read_config_file() -> Option<Result<Vec<u8>, std::io::Error>> {
    let mut candidates = Vec::new();
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(&xdg).join("spm/config.toml"));
        candidates.push(PathBuf::from(&xdg).join("spm.toml"));
    }
    if let Ok(home) = env::var("HOME") {
        candidates.push(PathBuf::from(&home).join(".config/spm/config.toml"));
        candidates.push(PathBuf::from(&home).join(".config/spm.toml"));
        candidates.push(PathBuf::from(&home).join(".spm.toml"));
    }
    for candidate in &candidates {
        match std::fs::read(candidate) {
            Ok(contents) => return Some(Ok(contents)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Some(Err(e)),
        }
    }
    None
}

fn load_config_from_env() -> Vec<(Vec<String>, String)> {
    env_to_config_overrides("SPM__")
}

fn env_to_config_overrides(prefix: &str) -> Vec<(Vec<String>, String)> {
    env::vars()
        .filter(|(key, _)| key.starts_with(prefix))
        .map(|(key, val)| {
            let name = &key[prefix.len()..];
            let path: Vec<String> = name
                .split("__")
                .map(|s| screaming_snake_to_camel(s))
                .collect();
            (path, val)
        })
        .collect()
}

fn screaming_snake_to_camel(s: &str) -> String {
    let binding = s.to_lowercase();
    let mut chars = binding.chars();
    let first = chars.next().unwrap_or_default().to_string();
    let rest: String = chars.collect();
    first + &rest
}

fn apply_overrides<I>(map: &mut TomlTable, overrides: I) -> Result<(), ConfigError>
where
    I: IntoIterator<Item = (Vec<String>, String)>,
{
    for (path, value) in overrides {
        let path_refs: Vec<&str> = path.iter().map(|s| s.as_str()).collect();
        // Try parsing as TOML first (for arrays, numbers, booleans)
        let toml_value: TomlValue = match toml::from_str(&value) {
            Ok(v) => v,
            Err(_) => TomlValue::String(value.clone()), // fallback to string
        };
        apply_override(map, &path_refs, &toml_value)?;
    }
    Ok(())
}

fn apply_override(
    map: &mut TomlTable,
    path: &[&str],
    value: &TomlValue,
) -> Result<(), ConfigError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.len() == 1 {
        map.insert(path[0].to_string(), value.clone());
        return Ok(());
    }
    let entry = map
        .entry(path[0].to_string())
        .or_insert(TomlValue::Table(TomlTable::new()));
    if let TomlValue::Table(m) = entry {
        apply_override(m, &path[1..], value)?;
    }
    Ok(())
}
