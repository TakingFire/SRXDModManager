use std::{sync::LazyLock, time::Duration};

use axum::{Json, Router, http::StatusCode, response::Redirect, routing::get};
use futures::future::join_all;
use indicatif::{MultiProgress, ProgressBar};
use model::Manifest;
use tokio::{self, time::interval};

use crate::template::{get_template_github, get_template_local};

mod template;

static PORT: LazyLock<String> = LazyLock::new(|| std::env::var("PORT").unwrap_or("8080".into()));

const BEPINEX_URL: &str =
    "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.5/BepInEx_win_x64_5.4.23.5.zip";

#[tokio::main]
async fn main() {
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
    let file = tokio::fs::read_to_string("assets/manifest.json")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let manifest: Manifest =
        serde_json::from_str(&file).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(manifest))
}

async fn build_manifest() -> anyhow::Result<()> {
    let template: Manifest = get_template_github()
        .await
        .inspect_err(|_| eprintln!("Using fallback template"))
        .unwrap_or(
            get_template_local()
                .await
                .inspect_err(|_| eprintln!("Failed to read template.json"))?,
        );

    let mut manifest = match tokio::fs::read_to_string("assets/manifest.json").await {
        Ok(str) => serde_json::from_str(&str).unwrap_or_default(),
        Err(_) => Manifest::default(),
    };

    println!("Loading plugins");

    for i in 0..manifest.mods.len() {
        let mod_entry = &manifest.mods[i];
        if template
            .mods
            .iter()
            .find(|template| template.id == mod_entry.id)
            .is_none()
        {
            println!("Removing {}", mod_entry.id);
            manifest.mods.swap_remove(i);
        }
    }

    for mod_template in template.mods {
        if manifest
            .mods
            .iter()
            .find(|entry| entry.id == mod_template.id)
            .is_none()
        {
            println!("Adding {}", mod_template.id);
            manifest.mods.push(mod_template);
        }
    }

    for mod_entry in &mut manifest.mods {
        mod_entry.url = format!(
            "https://github.com/{}/{}",
            mod_entry.author, mod_entry.repository
        );

        for category in &mod_entry.categories {
            if !manifest.categories.contains(category) {
                manifest.categories.push(category.clone());
            }
        }
    }

    let progress_bars = MultiProgress::new();
    let plugins_progress = progress_bars.add(ProgressBar::new(manifest.mods.len() as u64));
    let releases_progress = progress_bars.add(ProgressBar::new(0));

    let tasks = manifest.mods.iter_mut().map(|mod_entry| {
        let releases_progress = releases_progress.clone();
        let plugins_progress = plugins_progress.clone();
        async move {
            let _ = template::get_mod_releases(mod_entry, releases_progress).await;
            plugins_progress.inc(1);
        }
    });

    join_all(tasks).await;

    plugins_progress.finish_and_clear();
    releases_progress.finish_and_clear();

    manifest.categories.sort();

    tokio::fs::write("assets/manifest.json", serde_json::to_string(&manifest)?).await?;

    Ok(())
}
