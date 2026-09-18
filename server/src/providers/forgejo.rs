use axum::http::HeaderMap;
use model::Version;
use serde::Deserialize;
use std::{sync::LazyLock, time::Duration};

use crate::providers::{Forgejo, Provider, get_host_and_repo, hash_file};

pub type Releases = Vec<Release>;

#[derive(Deserialize, Debug)]
pub struct Release {
    pub tag_name: String,
    pub published_at: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Debug)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", "application/json".parse().unwrap());
    headers.insert("User-Agent", "TakingFire-SRXD-Mod-Server".parse().unwrap());

    reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
});

impl Provider for Forgejo {
    async fn get_versions(
        entry: &mut model::Mod,
        progress: Option<indicatif::ProgressBar>,
    ) -> anyhow::Result<()> {
        let (host, repository) = get_host_and_repo(&entry.url)?;

        let releases: Releases = CLIENT
            .get(format!(
                "https://{}/api/v1/repos{}/releases",
                host, repository
            ))
            .send()
            .await?
            .json()
            .await?;

        if let Some(progress) = &progress {
            progress.inc_length(releases.len() as u64);
        }

        for release in releases {
            if entry
                .versions
                .iter()
                .find(|v| release.tag_name == v.name)
                .is_some()
            {
                continue;
            }

            for asset in release.assets {
                if wildmatch::WildMatch::new(&entry.file).matches(&asset.name) {
                    let file = CLIENT
                        .get(asset.browser_download_url.clone())
                        .send()
                        .await?
                        .bytes()
                        .await?;

                    let sha256 = hash_file(file);

                    entry.versions.push(Version {
                        name: release.tag_name.clone(),
                        url: asset.browser_download_url.clone(),
                        created_at: release.published_at.clone(),
                        digest: sha256,
                    });
                }
            }

            if let Some(progress) = &progress {
                progress.inc(1);
            }
        }

        Ok(())
    }
}
