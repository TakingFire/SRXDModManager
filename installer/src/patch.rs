#![allow(clippy::redundant_closure_call)]

use std::{cmp::Ordering, time::Duration};

use configparser::ini::Ini;
use eframe::egui::{Color32, RichText};
use flume::Sender;
use model::Manifest;
use serde_json::Value;
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
    pub out_outdated: bool,
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
    pub fn default(msg: impl Into<String>) -> Self {
        Self::Default(msg.into())
    }

    pub fn success(msg: impl Into<String>) -> Self {
        Self::Success(msg.into())
    }

    pub fn warning(msg: impl Into<String>) -> Self {
        Self::Warning(msg.into())
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error(msg.into())
    }

    pub fn text(&self) -> RichText {
        match self {
            MessageType::Default(s) => RichText::new(s),
            MessageType::Success(s) => RichText::new(s).color(Color32::from_rgb(90, 170, 255)),
            MessageType::Warning(s) => RichText::new(s).color(Color32::from_rgb(255, 160, 80)),
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
        let _ = tx.send(StatusType::Message(MessageType::default(t!(
            "status.dirs_scan"
        ))));

        let result = || -> Result<(), MessageType> {
            ctx.out_directories.app_dir = Some(
                eframe::storage_dir("SRXD Mod Manager")
                    .ok_or(MessageType::error(t!("error.app_dir")))?,
            );

            let steam = steamlocate::SteamDir::locate()
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.steam_dir")))?;

            ctx.out_directories.steam_dir = Some(steam.path().to_path_buf());

            let (game, game_lib) = steam
                .find_app(GAME_ID)
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.game_dir")))?
                .ok_or(MessageType::error(t!("error.game_dir")))?;

            ctx.out_directories.game_dir = Some(game_lib.resolve_app_dir(&game));

            eprintln!(
                "App: {}\nSteam: {}\nGame: {}",
                ctx.out_directories
                    .app_dir
                    .as_ref()
                    .unwrap()
                    .to_string_lossy(),
                ctx.out_directories
                    .steam_dir
                    .as_ref()
                    .unwrap()
                    .to_string_lossy(),
                ctx.out_directories
                    .game_dir
                    .as_ref()
                    .unwrap()
                    .to_string_lossy()
            );

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
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.path_locate")))?
            {
                let _ = tx.send(StatusType::Message(MessageType::default(t!(
                    "status.patcher_found"
                ))));

                return Ok(());
            }

            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.patcher_dl"
            ))));

            let download = reqwest::get(PATCHER_URL)
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.download")))?;

            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                download
                    .bytes()
                    .await
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.file_read")))?,
            ))
            .inspect_err(|e| eprintln!("{}", e))
            .map_err(|_| MessageType::error(t!("error.zip_read")))?;

            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.zip_extract"
            ))));

            archive
                .extract(ctx.directories.app_dir.as_ref().unwrap())
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.zip_extract")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetPatcher(ctx), tx);
    });
}

pub fn get_manifest(mut ctx: GetManifestContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::default(t!(
            "status.mods_fetch"
        ))));

        let result = async || -> Result<(), MessageType> {
            let res = reqwest::get(MANIFEST_URL)
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.web_request")))?
                .text()
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|err| MessageType::error(err.to_string()))?;

            let manifest: Value = serde_json::from_str(&res)
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.file_read")))?;

            ctx.out_outdated = manifest
                .get("version")
                .and_then(|version| version.as_str())
                .map(|version| {
                    matches!(
                        natord::compare_ignore_case(version, &model::get_version()),
                        Ordering::Greater
                    )
                })
                .unwrap_or(false);

            ctx.out_manifest = serde_json::from_value(manifest)
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.file_read")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetManifest(ctx), tx);
    });
}

pub fn get_installed_mods(mut ctx: GetInstalledModsContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::default(t!(
            "status.mods_scan"
        ))));

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
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.path_create")))?;

            let mut entries = fs::read_dir(plugins_dir)
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.path_open")))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.file_read")))?
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
        let _ = tx.send(StatusType::Message(MessageType::default(t!(
            "status.mod_download",
            name = ctx.entry.entry.id
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
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.path_create")))?;

            let download = reqwest::get(version.url.to_owned())
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.download")))?;

            match plugin_name
                .split(".")
                .last()
                .ok_or(MessageType::error(t!("error.file_format")))?
            {
                "zip" => {
                    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                        download
                            .bytes()
                            .await
                            .inspect_err(|e| eprintln!("{}", e))
                            .map_err(|_| MessageType::error(t!("error.file_read")))?,
                    ))
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.zip_read")))?;

                    archive
                        .extract(&plugin_dir)
                        .inspect_err(|e| eprintln!("{}", e))
                        .map_err(|_| MessageType::error(t!("error.zip_extract")))?;
                }

                "dll" => {
                    fs::write(
                        &plugin_dir.join(plugin_name),
                        download
                            .bytes()
                            .await
                            .inspect_err(|e| eprintln!("{}", e))
                            .map_err(|_| MessageType::error(t!("error.file_read")))?,
                    )
                    .await
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.file_write")))?;
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
        let _ = tx.send(StatusType::Message(MessageType::default(t!(
            "status.mod_remove",
            name = ctx.entry.entry.id
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
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.file_delete")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::UninstallMod(ctx), tx);
    });
}

pub fn patch_game_files(ctx: PatchGameFilesContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.set_config"
            ))));

            let app_dir = ctx.directories.app_dir.as_ref().unwrap();
            let game_dir = ctx.directories.game_dir.as_ref().unwrap();
            #[allow(unused)]
            let steam_dir = ctx.directories.steam_dir.as_ref().unwrap();

            let base_dir = game_dir.join("UnityPlayer.dll");
            let base_renamed_dir = game_dir.join("UnityPlayer_IL2CPP.dll");
            let mono_dir = game_dir.join("UnityPlayer_Mono.dll");

            let mut doorstop = Ini::new();

            doorstop
                .load(app_dir.join("doorstop_config.ini"))
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.config_read")))?;

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
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.config_write")))?;

            #[cfg(target_os = "linux")]
            'regedit: {
                let _ = tx.send(StatusType::Message(MessageType::default(t!(
                    "status.override_set"
                ))));

                let reg_dir = steam_dir
                    .join("steamapps")
                    .join("compatdata")
                    .join(GAME_ID.to_string())
                    .join("pfx")
                    .join("user.reg");

                if !fs::try_exists(&reg_dir).await.unwrap_or(false) {
                    let _ = tx.send(StatusType::Message(MessageType::warning(t!(
                        "error.config_locate"
                    ))));

                    break 'regedit;
                }

                let reg = regashii::Registry::deserialize_file(&reg_dir)
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.config_read")))?
                    .with(
                        r"Software\Wine\DllOverrides",
                        regashii::Key::new()
                            .with("winhttp", regashii::Value::Sz("native,builtin"))
                            .with("*winhttp", regashii::Value::Sz("native,builtin")),
                    );

                reg.serialize_file(&reg_dir)
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.config_write")))?;
            }

            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.files_copy"
            ))));

            let base_dir_exists = fs::try_exists(&base_dir).await.unwrap_or(false);
            let base_renamed_dir_exists = fs::try_exists(&base_renamed_dir).await.unwrap_or(false);
            let mono_dir_exists = fs::try_exists(&mono_dir).await.unwrap_or(false);

            if !base_dir_exists {
                return Err(MessageType::error(t!("error.file_locate")));
            }

            if mono_dir_exists {
                fs::rename(&base_dir, &base_renamed_dir)
                    .await
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.file_rename")))?;

                fs::rename(&mono_dir, &base_dir)
                    .await
                    .inspect_err(|e| eprintln!("{}", e))
                    .map_err(|_| MessageType::error(t!("error.file_rename")))?;
            } else if !base_renamed_dir_exists {
                let _ = tx.send(StatusType::Message(MessageType::warning(t!(
                    "warning.unityplayer"
                ))));
            }

            fs::copy(
                app_dir.join("doorstop_config.ini"),
                game_dir.join("doorstop_config.ini"),
            )
            .await
            .inspect_err(|e| eprintln!("{}", e))
            .map_err(|_| MessageType::error(t!("error.file_copy")))?;

            fs::copy(app_dir.join("winhttp.dll"), game_dir.join("winhttp.dll"))
                .await
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.file_copy")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::PatchGameFiles(ctx), tx);
    });
}

pub fn unpatch_game_files(ctx: PatchGameFilesContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.files_remove"
            ))));

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
            let _ = tx.send(StatusType::Message(MessageType::success(t!(
                "status.launch_game"
            ))));

            let steam_dir = ctx.directories.steam_dir.as_ref().unwrap();

            #[cfg(target_os = "windows")]
            let launch_dir = steam_dir.join("steam.exe");
            #[cfg(target_os = "linux")]
            let launch_dir = "steam";

            let _process = std::process::Command::new(launch_dir)
                .args(["-applaunch", &GAME_ID.to_string()])
                .spawn()
                .inspect_err(|e| eprintln!("{}", e))
                .map_err(|_| MessageType::error(t!("error.launch_game")))?;

            tokio::time::sleep(Duration::from_secs(4)).await;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::LaunchGame(ctx), tx);
    });
}
