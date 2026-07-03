use std::{sync::LazyLock, time::Duration};

use axum::{Json, Router, http::StatusCode, response::Redirect, routing::get};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar};
use model::{Manifest, Mod};
use tokio::{self, time::interval};

use crate::{
    providers::{Forgejo, GitHub, Provider, ProviderType},
    template::{ModTemplate, Template, get_template_github, get_template_local},
};

mod providers;
mod template;

static PORT: LazyLock<String> = LazyLock::new(|| std::env::var("PORT").unwrap_or("8080".into()));

const BEPINEX_URL: &str =
    "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.5/BepInEx_win_x64_5.4.23.5.zip";

#[tokio::main]
async fn main() {
    println!("USING PORT {}", *PORT);

    build_manifest()
        .await
        .expect("Failed to create manifest.json");

    tokio::spawn(async {
        let mut interval = interval(Duration::from_mins(30));
        interval.tick().await;

        loop {
            interval.tick().await;
            let _ = build_manifest().await;
        }
    });

    println!("Starting Server");

    let app = Router::new()
        .route("/bepinex", get(get_bepinex))
        .route("/mods", get(get_mods));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", *PORT))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_bepinex() -> Redirect {
    Redirect::permanent(BEPINEX_URL)
}

async fn get_mods() -> Result<Json<Manifest>, StatusCode> {
    let file = tokio::fs::read_to_string("mods/manifest.json")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let manifest: Manifest =
        serde_json::from_str(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(manifest))
}

async fn build_manifest() -> anyhow::Result<()> {
    let template: Template = get_template_github()
        .await
        .inspect_err(|err| {
            eprintln!("{:?}", err);
            eprintln!("Using fallback template");
        })
        .unwrap_or(
            get_template_local()
                .await
                .inspect_err(|_| eprintln!("Failed to read template.json"))?,
        );

    let mut manifest = match tokio::fs::read_to_string("mods/manifest.json").await {
        Ok(str) => serde_json::from_str(&str).unwrap_or_default(),
        Err(_) => Manifest::default(),
    };

    println!("Loading plugins");

    let mut entry_map: Vec<(ModTemplate, Mod)> = template
        .mods
        .iter()
        .map(|mod_template| {
            let converted: Mod = mod_template.into();
            let mod_entry = manifest
                .mods
                .iter()
                .find(|entry| entry.id == converted.id)
                .map(|entry| Mod {
                    versions: entry.versions.clone(),
                    ..converted.clone()
                })
                .unwrap_or(converted);

            (mod_template.clone(), mod_entry)
        })
        .collect();

    for (_, mod_entry) in &mut entry_map {
        mod_entry
            .versions
            .sort_by(|a, b| b.created_at.cmp(&a.created_at));

        for category in &mod_entry.categories {
            if !manifest.categories.contains(category) {
                manifest.categories.push(category.clone());
            }
        }
    }

    let progress_bars = MultiProgress::new();
    let plugins_progress = progress_bars.add(ProgressBar::new(entry_map.len() as u64));
    let releases_progress = progress_bars.add(ProgressBar::new(0));

    stream::iter(entry_map.iter_mut())
        .for_each_concurrent(4, |(mod_template, mod_entry)| {
            let releases_progress = releases_progress.clone();
            let plugins_progress = plugins_progress.clone();
            async move {
                let result = match mod_template.provider {
                    ProviderType::GitHub => {
                        GitHub::get_versions(mod_entry, releases_progress).await
                    }
                    ProviderType::Forgejo => {
                        Forgejo::get_versions(mod_entry, releases_progress).await
                    }
                };

                if let Err(err) = result {
                    eprintln!("Failed to get versions for {}", mod_entry.id);
                    eprintln!("{:?}", err);
                }

                plugins_progress.inc(1);
            }
        })
        .await;

    plugins_progress.finish_and_clear();
    releases_progress.finish_and_clear();

    manifest.mods = entry_map
        .iter()
        .map(|(_, mod_entry)| mod_entry)
        .cloned()
        .collect();

    manifest.categories.sort();

    tokio::fs::write("mods/manifest.json", serde_json::to_string(&manifest)?).await?;

    Ok(())
}
