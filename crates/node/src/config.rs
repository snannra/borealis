use std::path::PathBuf;

pub struct NodeConfig {
    pub coordinator_url: String,
    pub key_path: PathBuf,
}

impl NodeConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let coordinator_url = std::env::var("COORDINATOR_URL")
            .expect("COORDINATOR_URL missing")
            .trim_end_matches('/')
            .to_owned();

        if coordinator_url.is_empty() {
            panic!("COORDINATOR_URL must not be empty");
        }

        let key_path = std::env::var_os("BOREALIS_KEY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".borealis.key"));

        Self {
            coordinator_url,
            key_path,
        }
    }
}
