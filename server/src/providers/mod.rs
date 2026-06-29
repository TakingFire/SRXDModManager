use anyhow::Result;
use indicatif::ProgressBar;
use model::Mod;
use serde::{Deserialize, Serialize};
use sha2::Digest;

pub mod github;

pub struct GitHub;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum ProviderType {
    #[default]
    GitHub,
}

pub trait Provider {
    async fn get_versions(entry: &mut Mod, progress: ProgressBar) -> Result<()>;
}

pub fn get_host_and_repo(url: &str) -> Result<(String, String)> {
    let url = url::Url::parse(url)?;
    Ok((url.host_str().unwrap_or_default().into(), url.path().into()))
}

pub fn hash_file(file: impl AsRef<[u8]>) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(file);
    hex::encode(hasher.finalize())
}
