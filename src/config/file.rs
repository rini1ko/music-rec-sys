//! `config.toml` (B5). Every value can also come from the command line or a
//! `MUSIC_*` environment variable; the priority is CLI > environment > file >
//! default. clap already merges the first two into one `Option`, so here the
//! CLI/env values are laid over the file ([`ServerSection::or`]) and the
//! defaults fill the rest ([`ServerSection::finish`]).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;

use crate::config::ServerSettings;
use crate::engine::EngineConfig;

pub const DEFAULT_PATH: &str = "config.toml";
const MAX_WEIGHT: f64 = 10.0;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {path}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} is not a valid config file")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("{0}")]
    Invalid(String),
}

/// The whole file. Unknown keys are an error, so a typo does not go unnoticed.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileConfig {
    pub server: ServerSection,
    pub engine: EngineSection,
}

/// `[server]`. Every field is optional: what is missing comes from elsewhere.
#[derive(Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSection {
    pub db: Option<String>,
    pub addr: Option<String>,
    pub log_dir: Option<String>,
    pub retrain_minutes: Option<u64>,
    pub cors_origins: Option<Vec<String>>,
    pub api_key: Option<String>,
    pub rate_per_second: Option<f64>,
    pub rate_burst: Option<u32>,
    pub request_timeout_secs: Option<u64>,
    pub body_limit_bytes: Option<usize>,
    pub cache_capacity: Option<usize>,
}

/// `[engine]`: the base (default or lenient) and the values that change it.
#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineSection {
    pub lenient: Option<bool>,
    pub trend_window_hours: Option<u32>,
    pub session_gap_minutes: Option<u32>,
    /// 1 or 2, see [`EngineConfig::markov_order`].
    pub markov_order: Option<usize>,
    pub weights: WeightsSection,
}

/// `[engine.weights]`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeightsSection {
    pub trends: Option<f64>,
    pub markov: Option<f64>,
    pub co_listening: Option<f64>,
    pub cf: Option<f64>,
    pub mf: Option<f64>,
}

/// Everything `serve` needs, with nothing left to decide.
#[derive(Debug, Clone, PartialEq)]
pub struct ServeConfig {
    pub db: String,
    pub addr: String,
    pub log_dir: String,
    pub retrain_minutes: u64,
    pub cors_origins: Vec<String>,
    pub settings: ServerSettings,
}

impl FileConfig {
    /// Reads and parses a config file.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Read`] if the file cannot be read, [`ConfigError::Parse`]
    /// if it is not TOML or has an unknown key or a value of the wrong type.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// `explicit` must exist; without it `config.toml` is read only if it is there.
    ///
    /// # Errors
    ///
    /// See [`FileConfig::load`].
    pub fn locate(explicit: Option<&str>) -> Result<Self, ConfigError> {
        match explicit {
            Some(path) => Self::load(Path::new(path)),
            None if Path::new(DEFAULT_PATH).exists() => Self::load(Path::new(DEFAULT_PATH)),
            None => Ok(Self::default()),
        }
    }
}

impl ServerSection {
    /// `self` where it has a value, `fallback` everywhere else.
    #[must_use]
    pub fn or(self, fallback: ServerSection) -> ServerSection {
        ServerSection {
            db: self.db.or(fallback.db),
            addr: self.addr.or(fallback.addr),
            log_dir: self.log_dir.or(fallback.log_dir),
            retrain_minutes: self.retrain_minutes.or(fallback.retrain_minutes),
            cors_origins: self.cors_origins.or(fallback.cors_origins),
            api_key: self.api_key.or(fallback.api_key),
            rate_per_second: self.rate_per_second.or(fallback.rate_per_second),
            rate_burst: self.rate_burst.or(fallback.rate_burst),
            request_timeout_secs: self.request_timeout_secs.or(fallback.request_timeout_secs),
            body_limit_bytes: self.body_limit_bytes.or(fallback.body_limit_bytes),
            cache_capacity: self.cache_capacity.or(fallback.cache_capacity),
        }
    }

    /// Fills the gaps with defaults and checks the ranges.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Invalid`] for a value out of its range.
    pub fn finish(self) -> Result<ServeConfig, ConfigError> {
        let defaults = ServerSettings::default();
        let rate_per_second = self.rate_per_second.unwrap_or(defaults.rate_per_second);
        if !(rate_per_second.is_finite() && rate_per_second >= 0.0) {
            return Err(invalid("rate_per_second must be 0 or more"));
        }
        let timeout = self
            .request_timeout_secs
            .unwrap_or(defaults.request_timeout.as_secs());
        if timeout == 0 {
            return Err(invalid("request_timeout_secs must be at least 1"));
        }
        let body_limit = self.body_limit_bytes.unwrap_or(defaults.body_limit);
        if body_limit < 1024 {
            return Err(invalid("body_limit_bytes must be at least 1024"));
        }
        // an empty key would let any request with an empty header through
        let api_key = self.api_key.filter(|key| !key.trim().is_empty());

        Ok(ServeConfig {
            db: self.db.unwrap_or_else(|| "data/music.db".to_string()),
            addr: self.addr.unwrap_or_else(|| "127.0.0.1:3000".to_string()),
            log_dir: self.log_dir.unwrap_or_else(|| "logs".to_string()),
            retrain_minutes: self.retrain_minutes.unwrap_or(0),
            cors_origins: self.cors_origins.unwrap_or_else(|| {
                vec![
                    "http://localhost:5173".to_string(),
                    "http://127.0.0.1:5173".to_string(),
                ]
            }),
            settings: ServerSettings {
                api_key,
                rate_per_second,
                rate_burst: self.rate_burst.unwrap_or(defaults.rate_burst),
                request_timeout: Duration::from_secs(timeout),
                body_limit,
                cache_capacity: self.cache_capacity.unwrap_or(defaults.cache_capacity),
            },
        })
    }
}

// written by hand so the key never ends up in a log line
impl fmt::Debug for ServerSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerSection")
            .field("db", &self.db)
            .field("addr", &self.addr)
            .field("api_key", &self.api_key.as_ref().map(|_| "<hidden>"))
            .finish_non_exhaustive()
    }
}

impl EngineSection {
    /// The engine config: lenient or default as the base (`lenient` from the
    /// command line wins over the file), then the values of this section.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Invalid`] for a weight outside 0..10 or a zero window or gap.
    pub fn engine_config(&self, lenient: Option<bool>) -> Result<EngineConfig, ConfigError> {
        let mut config = if lenient.or(self.lenient).unwrap_or(false) {
            EngineConfig::lenient()
        } else {
            EngineConfig::default()
        };
        if let Some(hours) = self.trend_window_hours {
            if hours == 0 {
                return Err(invalid("trend_window_hours must be at least 1"));
            }
            config.trend_window = chrono::Duration::hours(i64::from(hours));
        }
        if let Some(minutes) = self.session_gap_minutes {
            if minutes == 0 {
                return Err(invalid("session_gap_minutes must be at least 1"));
            }
            config.session_gap = chrono::Duration::minutes(i64::from(minutes));
        }

        if let Some(order) = self.markov_order {
            if !(1..=2).contains(&order) {
                return Err(invalid("markov_order must be 1 or 2"));
            }
            config.markov_order = order;
        }

        let weights = self.weights;
        for (name, value, slot) in [
            ("trends", weights.trends, &mut config.weights.trends),
            ("markov", weights.markov, &mut config.weights.markov),
            (
                "co_listening",
                weights.co_listening,
                &mut config.weights.co_listening,
            ),
            ("cf", weights.cf, &mut config.weights.cf),
            ("mf", weights.mf, &mut config.weights.mf),
        ] {
            if let Some(value) = value {
                if !(0.0..=MAX_WEIGHT).contains(&value) {
                    return Err(invalid(&format!(
                        "weight {name} must be between 0 and {MAX_WEIGHT}"
                    )));
                }
                *slot = value;
            }
        }
        Ok(config)
    }
}

fn invalid(message: &str) -> ConfigError {
    ConfigError::Invalid(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        [server]
        db = "data/big.db"
        rate_per_second = 5
        api_key = "from-file"

        [engine]
        lenient = true
        session_gap_minutes = 20

        [engine.weights]
        cf = 0.9
    "#;

    #[test]
    fn a_file_is_parsed_with_every_section() {
        let config: FileConfig = toml::from_str(SAMPLE).unwrap();
        assert_eq!(config.server.db.as_deref(), Some("data/big.db"));
        assert_eq!(config.server.rate_per_second, Some(5.0));
        assert_eq!(config.engine.weights.cf, Some(0.9));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let result: Result<FileConfig, _> = toml::from_str("[server]\nrate_per_secnd = 5\n");
        assert!(result.is_err());
    }

    #[test]
    fn command_line_wins_over_the_file_and_defaults_fill_the_rest() {
        let file: FileConfig = toml::from_str(SAMPLE).unwrap();
        let cli = ServerSection {
            db: Some("data/play.db".to_string()),
            ..ServerSection::default()
        };
        let serve = cli.or(file.server).finish().unwrap();

        assert_eq!(serve.db, "data/play.db");
        assert_eq!(serve.settings.rate_per_second, 5.0);
        assert_eq!(serve.settings.api_key.as_deref(), Some("from-file"));
        assert_eq!(serve.addr, "127.0.0.1:3000");
        assert_eq!(
            serve.settings.cache_capacity,
            ServerSettings::default().cache_capacity
        );
    }

    #[test]
    fn out_of_range_values_are_errors() {
        let negative = ServerSection {
            rate_per_second: Some(-1.0),
            ..ServerSection::default()
        };
        assert!(negative.finish().is_err());

        let engine = EngineSection {
            weights: WeightsSection {
                markov: Some(11.0),
                ..WeightsSection::default()
            },
            ..EngineSection::default()
        };
        assert!(engine.engine_config(None).is_err());
    }

    #[test]
    fn engine_values_change_the_base_config() {
        let file: FileConfig = toml::from_str(SAMPLE).unwrap();
        let config = file.engine.engine_config(None).unwrap();
        assert_eq!(config.session_gap, chrono::Duration::minutes(20));
        assert!((config.weights.cf - 0.9).abs() < 1e-9);
        // lenient from the file lowers the co-listening threshold
        assert_eq!(config.co_listening.min_cooccurrence, 1);

        let strict = file.engine.engine_config(Some(false)).unwrap();
        assert_ne!(strict.co_listening.min_cooccurrence, 1);
    }

    #[test]
    fn an_empty_key_means_no_key() {
        let section = ServerSection {
            api_key: Some("  ".to_string()),
            ..ServerSection::default()
        };
        assert_eq!(section.finish().unwrap().settings.api_key, None);
    }

    #[test]
    fn the_example_file_in_the_repository_is_valid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
        let config = FileConfig::load(&path).unwrap();
        config.engine.engine_config(None).unwrap();
        let serve = config.server.finish().unwrap();
        assert_eq!(serve.settings.api_key, None);
    }

    #[test]
    fn a_missing_explicit_file_is_an_error() {
        assert!(matches!(
            FileConfig::locate(Some("no/such/config.toml")),
            Err(ConfigError::Read { .. })
        ));
    }
}
