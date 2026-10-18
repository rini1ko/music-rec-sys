//! Settings of the HTTP layer: the API key, limits, timeouts and the cache.
//! The `serve` command fills them from the command line, the environment and
//! `config.toml` (see [`crate::config::file`]).

pub mod file;

use std::fmt;
use std::time::Duration;

/// A value that must not reach a log line: `Debug` prints `<hidden>`.
/// Read it with [`Secret::expose`] only where it is really used.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// The value itself.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The value, consuming the wrapper.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<hidden>")
    }
}

/// How the API protects itself. [`Default`] suits a local run.
#[derive(Clone, PartialEq)]
pub struct ServerSettings {
    /// Required in `x-api-key` for writes and service endpoints. `None` leaves them open.
    pub api_key: Option<String>,
    /// Requests per second one client may send on average. 0 turns the limit off.
    pub rate_per_second: f64,
    /// Requests one client may send at once before the average applies.
    pub rate_burst: u32,
    /// Longest time a handler may take before the client gets a 503.
    pub request_timeout: Duration,
    /// Largest request body, in bytes.
    pub body_limit: usize,
    /// Recommendation lists kept ready; 0 turns the cache off.
    pub cache_capacity: usize,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            api_key: None,
            rate_per_second: 20.0,
            rate_burst: 40,
            request_timeout: Duration::from_secs(30),
            body_limit: 64 * 1024,
            cache_capacity: 1024,
        }
    }
}

// written by hand so the key never ends up in a log line
impl fmt::Debug for ServerSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerSettings")
            .field("api_key", &self.api_key.as_ref().map(|_| "<hidden>"))
            .field("rate_per_second", &self.rate_per_second)
            .field("rate_burst", &self.rate_burst)
            .field("request_timeout", &self.request_timeout)
            .field("body_limit", &self.body_limit)
            .field("cache_capacity", &self.cache_capacity)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_hides_the_key() {
        let settings = ServerSettings {
            api_key: Some("secret-value".to_string()),
            ..ServerSettings::default()
        };
        let text = format!("{settings:?}");
        assert!(!text.contains("secret-value"));
        assert!(text.contains("<hidden>"));
    }

    #[test]
    fn a_secret_hides_itself_in_debug_output() {
        let secret = Secret::from("secret-value".to_string());
        assert_eq!(format!("{secret:?}"), "<hidden>");
        assert_eq!(format!("{:?}", Some(&secret)), "Some(<hidden>)");
        assert_eq!(secret.expose(), "secret-value");
    }
}
