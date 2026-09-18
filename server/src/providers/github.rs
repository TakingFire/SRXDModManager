use axum::http::HeaderMap;
use model::Version;
use serde::Deserialize;
use std::{sync::LazyLock, time::Duration};

use crate::providers::{GitHub, Provider, get_host_and_repo, hash_file};

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
    pub digest: Option<String>,
}

const INSTALLER_URL: &str =
    "https://api.github.com/repos/TakingFire/SRXDModManager/releases/latest";

pub static TOKEN: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("GH_SERVER_TOKEN").ok());

pub static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", "application/vnd.github+json".parse().unwrap());
    headers.insert("User-Agent", "TakingFire-SRXD-Mod-Server".parse().unwrap());
    headers.insert("X-GitHub-Api-Version", "2026-03-10".parse().unwrap());

    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .default_headers(headers)
        .build()
        .unwrap()
});

impl Provider for GitHub {
    async fn get_versions(
        entry: &mut model::Mod,
        progress: Option<indicatif::ProgressBar>,
    ) -> anyhow::Result<()> {
        if TOKEN.is_none() {
            eprintln!("Warning: GH_SERVER_TOKEN not set");
            return Err(anyhow::anyhow!(""));
        }

        let (host, repository) = get_host_and_repo(&entry.url)?;

        let releases: Releases = CLIENT
            .get(format!("https://api.{}/repos{}/releases", host, repository))
            .bearer_auth((*TOKEN).clone().unwrap())
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
                    let sha256: String;

                    if let Some(digest) = asset.digest {
                        sha256 = (&digest)[7..].into();
                    } else {
                        let file = CLIENT
                            .get(asset.browser_download_url.clone())
                            .bearer_auth((*TOKEN).clone().unwrap())
                            .send()
                            .await?
                            .bytes()
                            .await?;

                        sha256 = hash_file(file);
                    }

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

pub async fn get_installer_version() -> anyhow::Result<semver::Version> {
    let release: Release = CLIENT
        .get(INSTALLER_URL)
        .bearer_auth((*TOKEN).clone().unwrap())
        .send()
        .await?
        .json()
        .await?;

    let version = semver::Version::parse(
        release
            .tag_name
            .strip_prefix("v")
            .unwrap_or(&release.tag_name),
    )?;

    Ok(version)
}
