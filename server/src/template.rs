use anyhow::Result;
use axum::http::HeaderMap;
use indicatif::ProgressBar;
use model::{Manifest, Mod, Version};
use serde::Deserialize;
use sha2::Digest;
use std::{collections::HashMap, sync::LazyLock};

pub type DigestCache = HashMap<String, HashMap<String, String>>;

// GitHub Deserializer Types

pub type GHReleases = Vec<GHRelease>;

#[derive(Deserialize, Debug)]
pub struct GHRelease {
    // pub url: String,
    pub tag_name: String,
    pub published_at: String,
    pub assets: Vec<GHAsset>,
}

#[derive(Deserialize, Debug)]
pub struct GHAsset {
    pub name: String,
    pub browser_download_url: String,
    pub digest: Option<String>,
}

static GH_TOKEN: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("GH_SERVER_TOKEN").ok());

static GH_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    let mut gh_headers = HeaderMap::new();
    gh_headers.insert("Accept", "application/vnd.github+json".parse().unwrap());
    gh_headers.insert("User-Agent", "TakingFire-SRXD-Mod-Server".parse().unwrap());
    gh_headers.insert("X-GitHub-Api-Version", "2026-03-10".parse().unwrap());

    reqwest::Client::builder()
        .default_headers(gh_headers)
        .build()
        .unwrap()
});

pub async fn get_mod_releases(entry: &mut Mod, progress: ProgressBar) -> Result<()> {
    if GH_TOKEN.is_none() {
        eprintln!("Warning: GH_SERVER_TOKEN not set");
        return Err(anyhow::anyhow!(""));
    }

    let releases: GHReleases = GH_CLIENT
        .get(format!(
            "https://api.github.com/repos/{}/{}/releases",
            entry.author, entry.repository
        ))
        .bearer_auth(&(*GH_TOKEN.clone().unwrap()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    progress.set_length(releases.len() as u64);
    progress.set_position(0);

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
                    let file = GH_CLIENT
                        .get(asset.browser_download_url.clone())
                        .bearer_auth(&*GH_TOKEN.clone().unwrap())
                        .send()
                        .await?
                        .bytes()
                        .await?;

                    let mut hasher = sha2::Sha256::new();
                    hasher.update(file);
                    sha256 = hex::encode(hasher.finalize());
                }

                entry.versions.push(Version {
                    name: release.tag_name.clone(),
                    url: asset.browser_download_url.clone(),
                    created_at: release.published_at.clone(),
                    digest: sha256,
                });
            }
        }

        progress.inc(1);
    }

    entry
        .versions
        .sort_by(|a, b| natord::compare(&b.created_at, &a.created_at));

    Ok(())
}

#[allow(unused)]
pub async fn build_digest_cache(manifest: &Manifest) -> DigestCache {
    let mut entry_map: DigestCache = HashMap::new();

    for entry in &manifest.mods {
        let mut version_map: HashMap<String, String> = HashMap::new();

        for version in &entry.versions {
            version_map.insert(version.name.clone(), version.digest.clone());
        }

        entry_map.insert(entry.id.clone(), version_map);
    }

    entry_map
}
