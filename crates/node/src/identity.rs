use boringtun::x25519::{PublicKey, StaticSecret};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub struct Identity {
    private_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl Identity {
    pub fn load_or_generate(path: &Path) -> Result<Self, String> {
        let private_key = match fs::read(path) {
            Ok(bytes) => bytes.try_into().map_err(|bytes: Vec<u8>| {
                format!(
                    "identity key {} must contain exactly 32 bytes, found {}",
                    path.display(),
                    bytes.len()
                )
            })?,
            Err(error) if error.kind() == ErrorKind::NotFound => generate_key(path)?,
            Err(error) => {
                return Err(format!(
                    "failed to read identity key {}: {error}",
                    path.display()
                ));
            }
        };

        let secret = StaticSecret::from(private_key);
        let public_key = PublicKey::from(&secret).to_bytes();

        Ok(Self {
            private_key,
            public_key,
        })
    }

    pub fn private_key(&self) -> StaticSecret {
        StaticSecret::from(self.private_key)
    }
}

fn generate_key(path: &Path) -> Result<[u8; 32], String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create identity directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let mut private_key = [0u8; 32];
    getrandom::fill(&mut private_key)
        .map_err(|error| format!("failed to generate identity key: {error}"))?;

    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(&private_key).map_err(|error| {
                format!("failed to write identity key {}: {error}", path.display())
            })?;
            file.sync_all().map_err(|error| {
                format!("failed to persist identity key {}: {error}", path.display())
            })?;
            tracing::info!(path = %path.display(), "generated node identity");
            Ok(private_key)
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let bytes = fs::read(path).map_err(|read_error| {
                format!(
                    "identity key {} was created concurrently but could not be read: {read_error}",
                    path.display()
                )
            })?;
            bytes.try_into().map_err(|bytes: Vec<u8>| {
                format!(
                    "identity key {} must contain exactly 32 bytes, found {}",
                    path.display(),
                    bytes.len()
                )
            })
        }
        Err(error) => Err(format!(
            "failed to create identity key {}: {error}",
            path.display()
        )),
    }
}
