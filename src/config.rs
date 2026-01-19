use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

const CONFIG_DIR: &str = ".openworkers";
const CONFIG_FILE: &str = "config.json";
const DEFAULT_API_URL: &str = "https://dash.openworkers.com/api/v1";

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config directory not found")]
    HomeDirNotFound,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Alias '{0}' not found")]
    AliasNotFound(String),

    #[error("Alias '{0}' already exists. Use --force to overwrite")]
    AliasExists(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AliasConfig {
    Api {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        insecure: bool,
    },
    Db {
        database_url: String,
    },
}

impl AliasConfig {
    pub fn api(url: impl Into<String>, token: Option<String>, insecure: bool) -> Self {
        Self::Api {
            url: url.into(),
            token,
            insecure,
        }
    }

    pub fn db(database_url: impl Into<String>) -> Self {
        Self::Db {
            database_url: database_url.into(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Api { .. } => "api",
            Self::Db { .. } => "db",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub aliases: HashMap<String, AliasConfig>,
}

impl Default for Config {
    fn default() -> Self {
        let mut aliases = HashMap::new();
        aliases.insert(
            "default".to_string(),
            AliasConfig::api(DEFAULT_API_URL, None, false),
        );

        Self {
            version: 1,
            default: Some("default".to_string()),
            aliases,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::HomeDirNotFound)?;
        Ok(home.join(CONFIG_DIR))
    }

    pub fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join(CONFIG_FILE))
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let dir = Self::config_dir()?;

        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_alias(&self, name: &str) -> Option<&AliasConfig> {
        self.aliases.get(name)
    }

    pub fn set_alias(
        &mut self,
        name: impl Into<String>,
        config: AliasConfig,
        force: bool,
    ) -> Result<(), ConfigError> {
        let name = name.into();

        if !force && self.aliases.contains_key(&name) {
            return Err(ConfigError::AliasExists(name));
        }

        self.aliases.insert(name, config);
        Ok(())
    }

    pub fn remove_alias(&mut self, name: &str) -> Result<AliasConfig, ConfigError> {
        // If removing the default alias, clear the default
        if self.default.as_deref() == Some(name) {
            self.default = None;
        }

        self.aliases
            .remove(name)
            .ok_or_else(|| ConfigError::AliasNotFound(name.to_string()))
    }

    pub fn set_default(&mut self, name: &str) -> Result<(), ConfigError> {
        if !self.aliases.contains_key(name) {
            return Err(ConfigError::AliasNotFound(name.to_string()));
        }

        self.default = Some(name.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alias_config_api() {
        let alias = AliasConfig::api(
            "https://example.com/api",
            Some("token123".to_string()),
            false,
        );

        assert_eq!(alias.type_name(), "api");

        if let AliasConfig::Api {
            url,
            token,
            insecure,
        } = alias
        {
            assert_eq!(url, "https://example.com/api");
            assert_eq!(token, Some("token123".to_string()));
            assert!(!insecure);
        } else {
            panic!("Expected Api variant");
        }
    }

    #[test]
    fn test_alias_config_api_no_token() {
        let alias = AliasConfig::api("https://example.com/api", None, false);

        if let AliasConfig::Api { url, token, .. } = alias {
            assert_eq!(url, "https://example.com/api");
            assert!(token.is_none());
        } else {
            panic!("Expected Api variant");
        }
    }

    #[test]
    fn test_alias_config_api_insecure() {
        let alias = AliasConfig::api("https://dev.localhost/api", None, true);

        if let AliasConfig::Api { insecure, .. } = alias {
            assert!(insecure);
        } else {
            panic!("Expected Api variant");
        }
    }

    #[test]
    fn test_alias_config_db() {
        let alias = AliasConfig::db("postgres://user:pass@localhost/db");

        assert_eq!(alias.type_name(), "db");

        if let AliasConfig::Db { database_url } = alias {
            assert_eq!(database_url, "postgres://user:pass@localhost/db");
        } else {
            panic!("Expected Db variant");
        }
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();

        assert_eq!(config.version, 1);
        assert_eq!(config.default, Some("default".to_string()));
        assert!(config.aliases.contains_key("default"));

        let default = config.aliases.get("default").unwrap();
        assert_eq!(default.type_name(), "api");

        if let AliasConfig::Api { url, token, .. } = default {
            assert_eq!(url, DEFAULT_API_URL);
            assert!(token.is_none());
        }
    }

    #[test]
    fn test_get_alias() {
        let config = Config::default();

        assert!(config.get_alias("default").is_some());
        assert!(config.get_alias("nonexistent").is_none());
    }

    #[test]
    fn test_set_alias_new() {
        let mut config = Config::default();

        let result = config.set_alias(
            "prod",
            AliasConfig::api("https://prod.example.com", None, false),
            false,
        );

        assert!(result.is_ok());
        assert!(config.aliases.contains_key("prod"));
    }

    #[test]
    fn test_set_alias_exists_no_force() {
        let mut config = Config::default();

        let result = config.set_alias(
            "default",
            AliasConfig::api("https://other.com", None, false),
            false,
        );

        assert!(matches!(result, Err(ConfigError::AliasExists(_))));
    }

    #[test]
    fn test_set_alias_exists_with_force() {
        let mut config = Config::default();
        let new_url = "https://new.example.com";

        let result = config.set_alias("default", AliasConfig::api(new_url, None, false), true);

        assert!(result.is_ok());

        if let AliasConfig::Api { url, .. } = config.aliases.get("default").unwrap() {
            assert_eq!(url, new_url);
        }
    }

    #[test]
    fn test_remove_alias() {
        let mut config = Config::default();
        config
            .set_alias("test", AliasConfig::db("postgres://localhost/test"), false)
            .unwrap();

        let removed = config.remove_alias("test").unwrap();

        assert_eq!(removed.type_name(), "db");
        assert!(!config.aliases.contains_key("test"));
    }

    #[test]
    fn test_remove_alias_not_found() {
        let mut config = Config::default();

        let result = config.remove_alias("nonexistent");

        assert!(matches!(result, Err(ConfigError::AliasNotFound(_))));
    }

    #[test]
    fn test_remove_alias_clears_default() {
        let mut config = Config::default();

        assert_eq!(config.default, Some("default".to_string()));

        config.remove_alias("default").unwrap();

        assert!(config.default.is_none());
    }

    #[test]
    fn test_set_default() {
        let mut config = Config::default();
        config
            .set_alias(
                "prod",
                AliasConfig::api("https://prod.example.com", None, false),
                false,
            )
            .unwrap();

        let result = config.set_default("prod");

        assert!(result.is_ok());
        assert_eq!(config.default, Some("prod".to_string()));
    }

    #[test]
    fn test_set_default_not_found() {
        let mut config = Config::default();

        let result = config.set_default("nonexistent");

        assert!(matches!(result, Err(ConfigError::AliasNotFound(_))));
    }

    #[test]
    fn test_json_serialization_api() {
        let alias = AliasConfig::api(
            "https://example.com/api",
            Some("token123".to_string()),
            false,
        );

        let json = serde_json::to_string(&alias).unwrap();
        let parsed: AliasConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.type_name(), "api");

        if let AliasConfig::Api { url, token, .. } = parsed {
            assert_eq!(url, "https://example.com/api");
            assert_eq!(token, Some("token123".to_string()));
        }
    }

    #[test]
    fn test_json_serialization_db() {
        let alias = AliasConfig::db("postgres://localhost/db");

        let json = serde_json::to_string(&alias).unwrap();
        let parsed: AliasConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.type_name(), "db");

        if let AliasConfig::Db { database_url } = parsed {
            assert_eq!(database_url, "postgres://localhost/db");
        }
    }

    #[test]
    fn test_json_serialization_config() {
        let mut config = Config::default();
        config
            .set_alias("infra", AliasConfig::db("postgres://localhost/db"), false)
            .unwrap();

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.default, Some("default".to_string()));
        assert!(parsed.aliases.contains_key("default"));
        assert!(parsed.aliases.contains_key("infra"));
    }

    #[test]
    fn test_json_api_without_token_skips_field() {
        let alias = AliasConfig::api("https://example.com", None, false);

        let json = serde_json::to_string(&alias).unwrap();

        assert!(!json.contains("token"));
    }
}
