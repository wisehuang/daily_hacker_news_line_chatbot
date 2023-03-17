use config::{Config, File, FileFormat};

pub fn get_config(config_name: &str) -> String {
    let config_builder = Config::builder().add_source(File::new("config.toml", FileFormat::Toml));

    let config_value: String = match config_builder.build() {
        Ok(config) => config
            .get::<String>(config_name)
            .expect("Missing config_name in config file"),
        Err(e) => {
            panic!("{}", e);
        }
    };
    config_value
}
