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
    },
    Db {
        database_url: String,
    },
}

impl AliasConfig {
    pub fn api(url: impl Into<String>, token: Option<String>) -> Self {
        Self::Api {
            url: url.into(),
            token,
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
        aliases.insert("cloud".to_string(), AliasConfig::api(DEFAULT_API_URL, None));

        Self {
            version: 1,
            default: Some("cloud".to_string()),
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

    pub fn get_default_alias(&self) -> Option<(&String, &AliasConfig)> {
        self.default
            .as_ref()
            .and_then(|name| self.aliases.get(name).map(|config| (name, config)))
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
