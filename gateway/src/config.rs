//! Gateway boot configuration.
//!
//! The gateway loads a JSON config file at startup to decide which instruments
//! to register on the `Engine`. Runtime registration is handled separately via
//! the admin endpoint.

use std::fs;
use std::path::{Path, PathBuf};

use engine::commands::InstrumentId;
use serde::Deserialize;

/// Environment variable whose value, if set, points at a JSON config file.
pub const CONFIG_ENV_VAR: &str = "GATEWAY_CONFIG";

/// Default instruments when no config file is provided (dev convenience).
const DEFAULT_INSTRUMENTS: &[InstrumentId] = &[1];

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GatewayConfig {
    #[serde(default)]
    pub instruments: Vec<InstrumentId>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("config file {path} registers no instruments")]
    EmptyInstruments { path: PathBuf },
    #[error("instrument_id must be greater than 0 (found 0 in {path})")]
    InvalidInstrumentId { path: PathBuf },
}

impl GatewayConfig {
    /// Build a default config using the built-in dev instrument set.
    pub fn default_dev() -> Self {
        Self {
            instruments: DEFAULT_INSTRUMENTS.to_vec(),
        }
    }

    /// Load config from the given path; validates non-empty + no-zero-ids.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let parsed: GatewayConfig = serde_json::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        if parsed.instruments.is_empty() {
            return Err(ConfigError::EmptyInstruments {
                path: path.to_path_buf(),
            });
        }
        if parsed.instruments.iter().any(|id| *id == 0) {
            return Err(ConfigError::InvalidInstrumentId {
                path: path.to_path_buf(),
            });
        }
        Ok(parsed)
    }

    /// Resolve config from the [`CONFIG_ENV_VAR`] env var, falling back to
    /// [`GatewayConfig::default_dev`] when unset.
    pub fn from_env() -> Result<Self, ConfigError> {
        match std::env::var(CONFIG_ENV_VAR) {
            Ok(path) if !path.is_empty() => Self::load(Path::new(&path)),
            _ => Ok(Self::default_dev()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_config(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("gateway-config-{suffix}.json"));
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn loads_valid_config() {
        let path = write_temp_config(r#"{"instruments":[1,2,3]}"#);
        let config = GatewayConfig::load(&path).unwrap();
        assert_eq!(config.instruments, vec![1, 2, 3]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_instrument_list() {
        let path = write_temp_config(r#"{"instruments":[]}"#);
        let err = GatewayConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::EmptyInstruments { .. }));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_zero_instrument_id() {
        let path = write_temp_config(r#"{"instruments":[0,1]}"#);
        let err = GatewayConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidInstrumentId { .. }));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn parse_error_surfaces_path() {
        let path = write_temp_config("not-json");
        let err = GatewayConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn default_dev_has_one_instrument() {
        let config = GatewayConfig::default_dev();
        assert_eq!(config.instruments, vec![1]);
    }
}
