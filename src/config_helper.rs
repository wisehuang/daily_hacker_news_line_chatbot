use config::{Config, File, FileFormat};

pub fn get_config_by_file(config_name: &str, config_file: &str) -> String {
    let config_builder = Config::builder().add_source(File::new(config_file, FileFormat::Toml));

    let config_value: String = match config_builder.build() {
        Ok(config) => config
            .get::<String>(config_name)
            .expect(&format!("Missing {} in config file: {}", config_name, config_file)),
        Err(e) => {
            panic!("{}", e);
        }
    };
    config_value
}

pub fn get_config(config_name: &str) -> String {
    return get_config_by_file(config_name, "config.toml");
}

pub fn get_prompt(prompt: &str) -> String {
    return get_config_by_file(prompt, "prompts.toml");
}