use std::sync::OnceLock;
use config::{self as config_rs, Config as ConfigRS, File, FileFormat};
use std::path::Path;

/// Application-wide configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config: {0}")]
    ConfigReadError(#[from] config_rs::ConfigError),

    #[error("Config file '{0}' not found")]
    FileNotFound(String),

    #[error("Missing config value for key: {0}")]
    MissingKey(String),
}

/// Global config instance accessed through get_instance()
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Main configuration struct holding all application settings
pub struct Config {
    main_config: ConfigRS,
    secrets: ConfigRS,
    prompts: ConfigRS,
}

impl Config {
    /// Initialize the configuration from files
    pub fn new() -> Result<Self, ConfigError> {
        // Verify config files exist
        let files = [
            ("config.toml", "Main configuration"),
            ("secrets.toml", "API secrets"), 
            ("prompts.toml", "AI prompts"),
        ];
        
        for (file, desc) in files {
            if !Path::new(file).exists() {
                return Err(ConfigError::FileNotFound(format!("{} file: {}", desc, file)));
            }
        }

        let main_config = ConfigRS::builder()
            .add_source(File::new("config.toml", FileFormat::Toml))
            .build()?;
            
        let secrets = ConfigRS::builder()
            .add_source(File::new("secrets.toml", FileFormat::Toml))
            .build()?;
            
        let prompts = ConfigRS::builder()
            .add_source(File::new("prompts.toml", FileFormat::Toml))
            .build()?;

        Ok(Self {
            main_config,
            secrets,
            prompts,
        })
    }

    /// Get global singleton instance
    pub fn get_instance() -> &'static Self {
        CONFIG.get_or_init(|| {
            Self::new().unwrap_or_else(|e| {
                eprintln!("Failed to initialize configuration: {}", e);
                std::process::exit(1);
            })
        })
    }

    /// Get a value from the main config file
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        self.main_config.get::<T>(key)
            .map_err(|_| ConfigError::MissingKey(key.to_string()))
    }

    /// Get a value from the secrets file
    pub fn get_secret<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        self.secrets.get::<T>(key)
            .map_err(|_| ConfigError::MissingKey(key.to_string()))
    }

    /// Get a value from the prompts file
    pub fn get_prompt<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        self.prompts.get::<T>(key)
            .map_err(|_| ConfigError::MissingKey(key.to_string()))
    }
}

// Convenience functions for backward compatibility
pub fn get_config(config_name: &str) -> String {
    Config::get_instance().get(config_name)
        .unwrap_or_else(|e| {
            log::error!("Failed to get config value {}: {}", config_name, e);
            String::new()
        })
}

pub fn get_secret(secret_name: &str) -> String {
    Config::get_instance().get_secret(secret_name)
        .unwrap_or_else(|e| {
            log::error!("Failed to get secret value {}: {}", secret_name, e);
            String::new()
        })
}

pub fn get_prompt(prompt_name: &str) -> String {
    Config::get_instance().get_prompt(prompt_name)
        .unwrap_or_else(|e| {
            log::error!("Failed to get prompt {}: {}", prompt_name, e);
            String::new()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_config() {
        let config_value = get_config("chatgpt.model");
        assert!(!config_value.is_empty(), "Config value should not be empty");
    }

    #[test]
    fn test_get_prompt() {
        let prompt_value = get_prompt("prompt.summary_all");
        assert!(!prompt_value.is_empty(), "Prompt value should not be empty");
    }
} 