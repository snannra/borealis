use std::net::SocketAddr;

pub struct Config {
    pub database_url: String,
    pub bind_address: SocketAddr,
}

impl Config {
    pub fn from_env() -> Self {
        dotenv::dotenv().unwrap();

        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL missing")
            .parse::<String>()
            .expect("DATABASE_URL is invalid");

        let bind_address = std::env::var("BIND_ADDRESS")
            .expect("BIND_ADDRESS missing")
            .parse::<SocketAddr>()
            .expect("BIND_ADDRESS is invalid");

        Self {
            database_url,
            bind_address,
        }
    }
}
