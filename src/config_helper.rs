use config::{Config, File, FileFormat};

pub fn get_config_by_file(config_name: &str, config_file: &str) -> String {
    let config_builder = Config::builder().add_source(File::new(config_file, FileFormat::Toml));

    let config_value = config_builder.build().unwrap().get::<String>(config_name).map_err(|e| format!("Error reading config: {}", e)).unwrap();
    config_value
}

pub fn get_config(config_name: &str) -> String {
    return get_config_by_file(config_name, "config.toml");
}

pub fn get_secret(secret_name: &str) -> String {
    return get_config_by_file(secret_name, "secrets.toml");
}

pub fn get_prompt(prompt: &str) -> String {
    return get_config_by_file(prompt, "prompts.toml");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_config() {
        // Use the get_config_by_file function to read the data
        let config_value = get_config("chatgpt.model");

        // Assert that the returned value is correct
        assert_eq!(config_value, "gpt-4");
    }

    #[test]
    fn test_get_prompt() {
        // Use the get_prompt function to read the data
        let prompt_value = get_prompt("prompt.summary_all");

        // Assert that the returned value is not None or an empty string
        assert!(!prompt_value.is_empty(), "Prompt value is empty");
    }
}