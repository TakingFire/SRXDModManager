#![allow(clippy::redundant_closure_call)]

use std::time::Duration;

use configparser::ini::Ini;
use eframe::egui::{Color32, RichText};
use flume::Sender;
use model::Manifest;
use tokio::fs;

use crate::app::{DirectoryList, ModEntry, ModEntryState};

const MANIFEST_URL: &str = "https://srxd.bacur.xyz/mods";
const PATCHER_URL: &str = "https://srxd.bacur.xyz/bepinex";

const GAME_ID: u32 = 1058830;

#[allow(unused)]
pub enum TaskContext {
    GetDirectories(GetDirectoriesContext),
    GetPatcher(GetPatcherContext),
    GetManifest(GetManifestContext),
    GetInstalledMods(GetInstalledModsContext),
    InstallMod(InstallModContext),
    UninstallMod(InstallModContext),
    PatchGameFiles(PatchGameFilesContext),
    UnpatchGameFiles(PatchGameFilesContext),
    LaunchGame(LaunchGameContext),
}

#[derive(Default)]
pub struct GetDirectoriesContext {
    pub out_directories: DirectoryList,
}

pub struct GetPatcherContext {
    pub directories: DirectoryList,
}

#[derive(Default)]
pub struct GetManifestContext {
    pub out_manifest: Manifest,
}

pub struct GetInstalledModsContext {
    pub directories: DirectoryList,
    pub out_digest_list: Vec<String>,
}

pub struct InstallModContext {
    pub directories: DirectoryList,
    pub entry: ModEntry,
}

pub struct PatchGameFilesContext {
    pub directories: DirectoryList,
}

pub struct LaunchGameContext {
    pub directories: DirectoryList,
}

#[allow(unused)]
pub enum StatusType {
    Message(MessageType),
    Success(TaskContext),
    Error(TaskContext),
}

#[allow(unused)]
pub enum MessageType {
    Default(String),
    Success(String),
    Warning(String),
    Error(String),
}

impl MessageType {
    pub fn text(&self) -> RichText {
        match self {
            MessageType::Default(s) => RichText::new(s),
            MessageType::Success(s) => RichText::new(s).color(Color32::from_rgb(90, 170, 255)),
            MessageType::Warning(s) => RichText::new(s).color(Color32::YELLOW),
            MessageType::Error(s) => RichText::new(s).color(Color32::RED),
        }
    }
}

fn send_task_result(result: Result<(), MessageType>, ctx: TaskContext, tx: Sender<StatusType>) {
    match result {
        Ok(_) => {
            let _ = tx.send(StatusType::Success(ctx));
        }
        Err(msg) => {
            let _ = tx.send(StatusType::Message(msg));
            let _ = tx.send(StatusType::Error(ctx));
        }
    };
}

pub fn get_directories(mut ctx: GetDirectoriesContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::Default(
            "Locating Directories".into(),
        )));

        let result = || -> Result<(), MessageType> {
            ctx.out_directories.app_dir = Some(
                eframe::storage_dir("SRXD Mod Manager")
                    .ok_or(MessageType::Error("Unknown App Directory".into()))?,
            );

            let steam = steamlocate::SteamDir::locate()
                .map_err(|_| MessageType::Error("Unknown Steam Directory".into()))?;

            ctx.out_directories.steam_dir = Some(steam.path().to_path_buf());

            let (game, game_lib) = steam
                .find_app(GAME_ID)
                .map_err(|_| MessageType::Error("Unknown Game Directory".into()))?
                .ok_or(MessageType::Error("Unknown Game Directory".into()))?;

            ctx.out_directories.game_dir = Some(game_lib.resolve_app_dir(&game));

            Ok(())
        }();

        send_task_result(result, TaskContext::GetDirectories(ctx), tx);
    });
}

pub fn get_patcher(ctx: GetPatcherContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let patcher_dir = ctx.directories.app_dir.as_ref().unwrap().join("BepInEx");

            if fs::try_exists(patcher_dir)
                .await
                .map_err(|_| MessageType::Error("Failed to Read Path".into()))?
            {
                let _ = tx.send(StatusType::Message(MessageType::Default(
                    "Found Patcher".into(),
                )));

                return Ok(());
            }

            let _ = tx.send(StatusType::Message(MessageType::Default(
                "Downloading Patcher".into(),
            )));

            let download = reqwest::get(PATCHER_URL)
                .await
                .map_err(|_| MessageType::Error("Download Failed".into()))?;

            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                download
                    .bytes()
                    .await
                    .map_err(|_| MessageType::Error("Failed to Read Data".into()))?,
            ))
            .map_err(|_| MessageType::Error("Failed to Read Archive".into()))?;

            let _ = tx.send(StatusType::Message(MessageType::Default(
                "Extracting Archive".into(),
            )));

            archive
                .extract(ctx.directories.app_dir.as_ref().unwrap())
                .map_err(|_| MessageType::Error("Failed to Extract Archive".into()))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetPatcher(ctx), tx);
    });
}

pub fn get_manifest(mut ctx: GetManifestContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::Default(
            "Fetching Mod List".into(),
        )));

        let result = async || -> Result<(), MessageType> {
            let res = reqwest::get(MANIFEST_URL)
                .await
                .map_err(|_| MessageType::Error("Server Request Failed".into()))?
                .text()
                .await
                .map_err(|err| MessageType::Error(err.to_string()))?;

            ctx.out_manifest = serde_json::from_str(&res)
                .map_err(|_| MessageType::Error("Failed to Read Mod List".into()))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetManifest(ctx), tx);
    });
}

pub fn get_installed_mods(mut ctx: GetInstalledModsContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::Default(
            "Scanning Directory".into(),
        )));

        let result = async || -> Result<(), MessageType> {
            let plugins_dir = ctx
                .directories
                .app_dir
                .as_ref()
                .unwrap()
                .join("BepInEx")
                .join("plugins");

            fs::create_dir_all(&plugins_dir)
                .await
                .map_err(|_| MessageType::Error("Failed to Create Folder".into()))?;

            let mut entries = fs::read_dir(plugins_dir)
                .await
                .map_err(|_| MessageType::Error("Failed to Open Folder".into()))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|_| MessageType::Error("Failed to Read Mods".into()))?
            {
                ctx.out_digest_list
                    .push(entry.file_name().to_string_lossy().into());
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetInstalledMods(ctx), tx);
    });
}

pub fn install_mod(ctx: InstallModContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::Default(format!(
            "Downloading {}",
            ctx.entry.entry.id
        ))));

        let result = async || -> Result<(), MessageType> {
            let version = &ctx.entry.entry.versions[ctx.entry.selected_version];

            let plugin_name = ctx.entry.entry.file.replace("*", "");
            let plugin_dir = ctx
                .directories
                .app_dir
                .as_ref()
                .unwrap()
                .join("BepInEx")
                .join("plugins")
                .join(&version.digest);

            fs::create_dir_all(&plugin_dir)
                .await
                .map_err(|_| MessageType::Error("Failed to Read Path".into()))?;

            let download = reqwest::get(version.url.to_owned())
                .await
                .map_err(|_| MessageType::Error("Download Failed".into()))?;

            match plugin_name
                .split(".")
                .last()
                .ok_or(MessageType::Error("Unknown File Format".into()))?
            {
                "zip" => {
                    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                        download
                            .bytes()
                            .await
                            .map_err(|_| MessageType::Error("Failed to Read Data".into()))?,
                    ))
                    .map_err(|_| MessageType::Error("Failed to Read Archive".into()))?;

                    archive
                        .extract(&plugin_dir)
                        .map_err(|_| MessageType::Error("Failed to Extract Archive".into()))?;
                }

                "dll" => {
                    fs::write(
                        &plugin_dir.join(plugin_name),
                        download
                            .bytes()
                            .await
                            .map_err(|_| MessageType::Error("Failed to Read Data".into()))?,
                    )
                    .await
                    .map_err(|_| MessageType::Error("Failed to Write Data".into()))?;
                }
                _ => {}
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::InstallMod(ctx), tx);
    });
}

pub fn uninstall_mod(ctx: InstallModContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::Default(format!(
            "Removing {}",
            ctx.entry.entry.id
        ))));

        let result = async || -> Result<(), MessageType> {
            let current_version = match ctx.entry.state {
                ModEntryState::PendingVersionChangeFrom(v) => v,
                _ => ctx.entry.selected_version,
            };

            let version = &ctx.entry.entry.versions[current_version];

            let plugin_dir = ctx
                .directories
                .app_dir
                .as_ref()
                .unwrap()
                .join("BepInEx")
                .join("plugins")
                .join(&version.digest);

            fs::remove_dir_all(&plugin_dir)
                .await
                .map_err(|_| MessageType::Error("Failed to Remove File".into()))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::UninstallMod(ctx), tx);
    });
}

pub fn patch_game_files(ctx: PatchGameFilesContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::Default(
                "Setting Config".into(),
            )));

            let app_dir = ctx.directories.app_dir.as_ref().unwrap();
            let game_dir = ctx.directories.game_dir.as_ref().unwrap();

            let base_dir = game_dir.join("UnityPlayer.dll");
            let base_renamed_dir = game_dir.join("UnityPlayer_IL2CPP.dll");
            let mono_dir = game_dir.join("UnityPlayer_Mono.dll");

            let mut doorstop = Ini::new();

            doorstop
                .load(app_dir.join("doorstop_config.ini"))
                .map_err(|_| MessageType::Error("Failed to Read Config".into()))?;

            doorstop.set(
                "General",
                "target_assembly",
                Some(
                    app_dir
                        .join("BepInEx/core/BepInEx.Preloader.dll")
                        .to_string_lossy()
                        .into(),
                ),
            );

            doorstop
                .write(app_dir.join("doorstop_config.ini"))
                .map_err(|_| MessageType::Error("Failed to Write Config".into()))?;

            let _ = tx.send(StatusType::Message(MessageType::Default(
                "Copying Files".into(),
            )));

            let base_dir_exists = fs::try_exists(&base_dir).await.unwrap_or(false);
            let base_renamed_dir_exists = fs::try_exists(&base_renamed_dir).await.unwrap_or(false);
            let mono_dir_exists = fs::try_exists(&mono_dir).await.unwrap_or(false);

            if !base_dir_exists {
                return Err(MessageType::Error("Failed to Find UnityPlayer".into()));
            }

            if mono_dir_exists {
                fs::rename(&base_dir, &base_renamed_dir)
                    .await
                    .map_err(|_| MessageType::Error("Failed to Rename UnityPlayer".into()))?;

                fs::rename(&mono_dir, &base_dir)
                    .await
                    .map_err(|_| MessageType::Error("Failed to Rename UnityPlayer".into()))?;
            } else if !base_renamed_dir_exists {
                let _ = tx.send(StatusType::Message(MessageType::Warning(
                    "UnityPlayers in Unexpected State".into(),
                )));
            }

            fs::copy(
                app_dir.join("doorstop_config.ini"),
                game_dir.join("doorstop_config.ini"),
            )
            .await
            .map_err(|_| MessageType::Error("Failed to Copy File".into()))?;

            fs::copy(app_dir.join("winhttp.dll"), game_dir.join("winhttp.dll"))
                .await
                .map_err(|_| MessageType::Error("Failed to Copy File".into()))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::PatchGameFiles(ctx), tx);
    });
}

pub fn unpatch_game_files(ctx: PatchGameFilesContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::Default(
                "Removing Patch Files".into(),
            )));

            let game_dir = ctx.directories.game_dir.as_ref().unwrap();

            let _ = fs::remove_file(game_dir.join("doorstop_config.ini")).await;
            let _ = fs::remove_file(game_dir.join("winhttp.dll")).await;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::PatchGameFiles(ctx), tx);
    });
}

pub fn launch_game(ctx: LaunchGameContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::Default(
                "Launching Game".into(),
            )));

            let steam_dir = ctx.directories.steam_dir.as_ref().unwrap();

            let _process = std::process::Command::new(steam_dir.join("steam.exe"))
                .env("WINEDLLOVERRIDES", "winhttp=b,n")
                .args(["-applaunch", &GAME_ID.to_string()])
                .spawn()
                .map_err(|_| MessageType::Error("Failed to Launch Game".into()))?;

            tokio::time::sleep(Duration::from_secs(4)).await;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::LaunchGame(ctx), tx);
    });
}
