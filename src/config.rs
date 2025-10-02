use serde::Deserialize;

lazy_static::lazy_static! {
    pub static ref APP_CONFIG: AppConfig = load_config_env::<AppConfig>();
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub csv_file: String,
    pub elasticsearch_url: String,
    pub elasticsearch_index: String,
    pub batch_size: usize,
    pub workers: usize,
    pub timeout_secs: u64,
}

/// Read config environment variables from .env file, then override them with envy
fn load_config_env<T: serde::de::DeserializeOwned>() -> T {
    dotenvy::dotenv().ok();
    envy::from_env().unwrap()
}
