#![allow(clippy::redundant_closure_call)]

use std::time::Duration;

use colored::Colorize;
use configparser::ini::Ini;
use eframe::egui::{Color32, RichText};
use flume::Sender;
use model::Manifest;
use tokio::{fs, time::error::Elapsed};

use crate::app::{DirectoryList, ModEntry, ModEntryState};

const MANIFEST_URL: &str = "https://srxd.bacur.xyz/mods";
const PATCHER_URL: &str = "https://srxd.bacur.xyz/bepinex";

const GAME_ID: u32 = 1058830;

#[allow(unused)]
pub enum TaskContext {
    GetDirectories(DirectoryList),
    GetPatcher(DirectoryList),
    GetManifest(GetManifestContext),

    GetInstalledMods(GetInstalledModsContext),
    GetExistingConfig(DirectoryList),
    CopyExistingConfig(DirectoryList),

    InstallMod(InstallModContext),
    UninstallMod(InstallModContext),

    PatchGameFiles(PatchGameFilesContext),
    UnpatchGameFiles(DirectoryList),
    LaunchGame(DirectoryList),
}

#[derive(Default)]
pub struct GetManifestContext {
    pub out_manifest: Manifest,
    pub out_outdated: bool,
    pub out_update: Option<String>,
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
    pub show_console: bool,
}

#[allow(unused)]
pub enum StatusType {
    Message(MessageType),
    Success(TaskContext),
    Error(TaskContext),
}

#[allow(unused)]
#[derive(Debug)]
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

    pub fn rich_text(&self) -> RichText {
        match self {
            Self::Default(s) => RichText::new(s),
            Self::Success(s) => RichText::new(s).color(Color32::from_rgb(90, 170, 255)),
            Self::Warning(s) => RichText::new(s).color(Color32::from_rgb(255, 160, 80)),
            Self::Error(s) => RichText::new(s).color(Color32::RED),
        }
    }

    pub fn colored_text(&self) -> colored::ColoredString {
        match self {
            Self::Default(s) => s.normal(),
            Self::Success(s) => s.bright_blue(),
            Self::Warning(s) => s.bright_yellow(),
            Self::Error(s) => s.bright_red(),
        }
    }
}

fn send_task_result(result: Result<(), MessageType>, ctx: TaskContext, tx: &Sender<StatusType>) {
    match result {
        Ok(()) => {
            let _ = tx.send(StatusType::Success(ctx));
        }
        Err(msg) => {
            let _ = tx.send(StatusType::Message(msg));
            let _ = tx.send(StatusType::Error(ctx));
        }
    }
}

pub fn get_directories(mut ctx: DirectoryList, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let _ = tx.send(StatusType::Message(MessageType::default(t!(
            "status.dirs_scan"
        ))));

        let result = || -> Result<(), MessageType> {
            ctx.app_dir = Some(
                eframe::storage_dir("SRXD Mod Manager")
                    .ok_or_else(|| MessageType::error(t!("error.app_dir")))?,
            );

            let steam = steamlocate::SteamDir::locate()
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.steam_dir")))?;

            ctx.steam_dir = Some(steam.path().to_path_buf());

            let (game, game_lib) = steam
                .find_app(GAME_ID)
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.game_dir")))?
                .ok_or_else(|| MessageType::error(t!("error.game_dir")))?;

            ctx.game_dir = Some(game_lib.resolve_app_dir(&game));

            eprintln!(
                "App: {}\nSteam: {}\nGame: {}\n",
                ctx.app_dir.as_ref().unwrap().to_string_lossy(),
                ctx.steam_dir.as_ref().unwrap().to_string_lossy(),
                ctx.game_dir.as_ref().unwrap().to_string_lossy()
            );

            Ok(())
        }();

        send_task_result(result, TaskContext::GetDirectories(ctx), &tx);
    });
}

pub fn get_patcher(ctx: DirectoryList, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let patcher_dir = ctx.app_dir.as_ref().unwrap().join("BepInEx");

            if fs::try_exists(patcher_dir)
                .await
                .inspect_err(|e| eprintln!("{e}"))
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
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.download")))?;

            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                download
                    .bytes()
                    .await
                    .inspect_err(|e| eprintln!("{e}"))
                    .map_err(|_| MessageType::error(t!("error.file_read")))?,
            ))
            .inspect_err(|e| eprintln!("{e}"))
            .map_err(|_| MessageType::error(t!("error.zip_read")))?;

            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.zip_extract"
            ))));

            archive
                .extract(ctx.app_dir.as_ref().unwrap())
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.zip_extract")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetPatcher(ctx), &tx);
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
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.web_request")))?
                .text()
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|err| MessageType::error(err.to_string()))?;

            #[derive(serde::Deserialize)]
            struct Header {
                version: String,
            }

            let header: Header = serde_json::from_str(&res)
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.file_read")))?;

            let own_api_version = semver::Version::parse(&model::get_version()).unwrap();
            if let Ok(server_api_version) = semver::Version::parse(&header.version) {
                ctx.out_outdated = server_api_version.major > own_api_version.major;
            }

            ctx.out_manifest = serde_json::from_str(&res)
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.file_read")))?;

            let own_app_version = semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
            if let Ok(server_app_version) = semver::Version::parse(&ctx.out_manifest.app_version)
                && server_app_version > own_app_version
            {
                ctx.out_update = Some(server_app_version.to_string());
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetManifest(ctx), &tx);
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
                .join("BepInEx/plugins");

            fs::create_dir_all(&plugins_dir)
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.path_create")))?;

            let mut entries = fs::read_dir(plugins_dir)
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.path_open")))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.file_read")))?
            {
                ctx.out_digest_list
                    .push(entry.file_name().to_string_lossy().into());
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetInstalledMods(ctx), &tx);
    });
}

pub fn get_existing_config(ctx: DirectoryList, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let game_config_dir = ctx.game_dir.as_ref().unwrap().join("BepInEx/config");

            fs::try_exists(&game_config_dir)
                .await
                .map_err(|_| MessageType::default(""))?
                .ok_or_else(|| MessageType::default(t!("")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::GetExistingConfig(ctx), &tx);
    });
}

pub fn copy_existing_config(ctx: DirectoryList, tx: Sender<StatusType>) {
    let _ = tx.send(StatusType::Message(MessageType::default(t!(
        "status.files_copy"
    ))));

    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let app_config_dir = ctx.app_dir.as_ref().unwrap().join("BepInEx/config");
            let game_config_dir = ctx.game_dir.as_ref().unwrap().join("BepInEx/config");

            copy_dir_all(game_config_dir, app_config_dir)
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.file_copy")))?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::CopyExistingConfig(ctx), &tx);
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

            let plugin_name = ctx.entry.entry.file.replace('*', "");
            let plugin_dir = ctx
                .directories
                .app_dir
                .as_ref()
                .unwrap()
                .join("BepInEx/plugins")
                .join(&version.digest);

            fs::create_dir_all(&plugin_dir)
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.path_create")))?;

            let download = reqwest::get(version.url.clone())
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.download")))?;

            match plugin_name
                .split('.')
                .next_back()
                .ok_or_else(|| MessageType::error(t!("error.file_format")))?
            {
                "zip" => {
                    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
                        download
                            .bytes()
                            .await
                            .inspect_err(|e| eprintln!("{e}"))
                            .map_err(|_| MessageType::error(t!("error.file_read")))?,
                    ))
                    .inspect_err(|e| eprintln!("{e}"))
                    .map_err(|_| MessageType::error(t!("error.zip_read")))?;

                    archive
                        .extract(&plugin_dir)
                        .inspect_err(|e| eprintln!("{e}"))
                        .map_err(|_| MessageType::error(t!("error.zip_extract")))?;
                }

                "dll" => {
                    fs::write(
                        &plugin_dir.join(plugin_name),
                        download
                            .bytes()
                            .await
                            .inspect_err(|e| eprintln!("{e}"))
                            .map_err(|_| MessageType::error(t!("error.file_read")))?,
                    )
                    .await
                    .inspect_err(|e| eprintln!("{e}"))
                    .map_err(|_| MessageType::error(t!("error.file_write")))?;
                }
                _ => {}
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::InstallMod(ctx), &tx);
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
                .join("BepInEx/plugins")
                .join(&version.digest);

            let metadata = fs::metadata(&plugin_dir)
                .await
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.path_locate")))?;

            if metadata.is_dir() {
                fs::remove_dir_all(&plugin_dir)
                    .await
                    .inspect_err(|e| eprintln!("{e}"))
                    .map_err(|_| MessageType::error(t!("error.file_delete")))?;
            } else {
                fs::remove_file(&plugin_dir)
                    .await
                    .inspect_err(|e| eprintln!("{e}"))
                    .map_err(|_| MessageType::error(t!("error.file_delete")))?;
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::UninstallMod(ctx), &tx);
    });
}

pub fn patch_game_files(ctx: PatchGameFilesContext, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.config_set"
            ))));

            write_doorstop_config(&ctx.directories)?;

            write_bepinex_config(&ctx.directories, ctx.show_console).await?;

            #[cfg(target_os = "linux")]
            write_proton_override(&ctx.directories, &tx).await?;

            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.files_copy"
            ))));

            rename_unity_player(&ctx.directories, &tx).await?;

            copy_patch_files(&ctx.directories).await?;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::PatchGameFiles(ctx), &tx);
    });
}

pub fn unpatch_game_files(ctx: DirectoryList, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::default(t!(
                "status.files_remove"
            ))));

            let game_dir = ctx.game_dir.as_ref().unwrap();

            let _ = fs::remove_file(game_dir.join("doorstop_config.ini")).await;
            let _ = fs::remove_file(game_dir.join("winhttp.dll")).await;

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::UnpatchGameFiles(ctx), &tx);
    });
}

pub fn launch_game(ctx: DirectoryList, tx: Sender<StatusType>) {
    tokio::spawn(async move {
        let result = async || -> Result<(), MessageType> {
            let _ = tx.send(StatusType::Message(MessageType::success(t!(
                "status.launch_game"
            ))));

            let steam_dir = ctx.steam_dir.as_ref().unwrap();

            #[cfg(target_os = "windows")]
            let launch_dir = steam_dir.join("steam.exe");
            #[cfg(target_os = "linux")]
            let launch_dir = "steam";

            let _process = std::process::Command::new(launch_dir)
                .args(["-applaunch", &GAME_ID.to_string()])
                .spawn()
                .inspect_err(|e| eprintln!("{e}"))
                .map_err(|_| MessageType::error(t!("error.launch_game")))?;

            let result = wait_for_process("SpinRhythm.exe", Duration::from_secs(15)).await;

            if result.is_ok() {
                let _ = tx.send(StatusType::Message(MessageType::success(t!(
                    "status.ready"
                ))));
            } else {
                let _ = tx.send(StatusType::Message(MessageType::warning(t!(
                    "warning.timed_out"
                ))));
            }

            Ok(())
        }()
        .await;

        send_task_result(result, TaskContext::LaunchGame(ctx), &tx);
    });
}

fn write_doorstop_config(ctx: &DirectoryList) -> Result<(), MessageType> {
    let app_dir = ctx.app_dir.as_ref().unwrap();
    let config_dir = app_dir.join("doorstop_config.ini");

    let mut doorstop = Ini::new();

    doorstop
        .load(&config_dir)
        .inspect_err(|e| eprintln!("{e}"))
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
        .write(&config_dir)
        .inspect_err(|e| eprintln!("{e}"))
        .map_err(|_| MessageType::error(t!("error.config_write")))?;

    Ok(())
}

async fn write_bepinex_config(ctx: &DirectoryList, show_console: bool) -> Result<(), MessageType> {
    let app_dir = ctx.app_dir.as_ref().unwrap();
    let config_dir = app_dir.join("BepInEx/config/BepInEx.cfg");

    let mut bepinex = Ini::new();

    let mut defaults = bepinex.defaults();
    defaults.case_sensitive = true;
    bepinex.load_defaults(defaults);

    if fs::try_exists(&config_dir).await.unwrap_or(false) {
        bepinex
            .load(&config_dir)
            .inspect_err(|e| eprintln!("{e}"))
            .map_err(|_| MessageType::error(t!("error.config_read")))?;
    } else {
        fs::create_dir_all(&config_dir.parent().unwrap())
            .await
            .inspect_err(|e| eprintln!("{e}"))
            .map_err(|_| MessageType::error(t!("error.path_create")))?;
    }

    bepinex.set("Logging.Console", "Enabled", Some(show_console.to_string()));

    bepinex
        .write(&config_dir)
        .inspect_err(|e| eprintln!("{e}"))
        .map_err(|_| MessageType::error(t!("error.config_write")))?;

    Ok(())
}

#[cfg(target_os = "linux")]
async fn write_proton_override(
    ctx: &DirectoryList,
    tx: &Sender<StatusType>,
) -> Result<(), MessageType> {
    let steam_dir = ctx.steam_dir.as_ref().unwrap();

    let _ = tx.send(StatusType::Message(MessageType::default(t!(
        "status.override_set"
    ))));

    let reg_dir = steam_dir
        .join("steamapps/compatdata")
        .join(GAME_ID.to_string())
        .join("pfx/user.reg");

    fs::try_exists(&reg_dir)
        .await
        .inspect_err(|e| eprintln!("{e}"))
        .map_err(|_| MessageType::warning(t!("error.config_locate")))?
        .ok_or_else(|| MessageType::warning(t!("error.config_locate")))?;

    let reg = regashii::Registry::deserialize_file(&reg_dir)
        .inspect_err(|e| eprintln!("{e}"))
        .map_err(|_| MessageType::error(t!("error.config_read")))?
        .with(
            r"Software\Wine\DllOverrides",
            regashii::Key::new()
                .with("winhttp", regashii::Value::Sz("native,builtin".to_owned()))
                .with("*winhttp", regashii::Value::Sz("native,builtin".to_owned())),
        );

    reg.serialize_file(&reg_dir)
        .inspect_err(|e| eprintln!("{e}"))
        .map_err(|_| MessageType::error(t!("error.config_write")))?;

    Ok(())
}

async fn rename_unity_player(
    ctx: &DirectoryList,
    tx: &Sender<StatusType>,
) -> Result<(), MessageType> {
    let game_dir = ctx.game_dir.as_ref().unwrap();

    let base_dir = game_dir.join("UnityPlayer.dll");
    let base_renamed_dir = game_dir.join("UnityPlayer_IL2CPP.dll");
    let mono_dir = game_dir.join("UnityPlayer_Mono.dll");

    let base_dir_exists = fs::try_exists(&base_dir).await.unwrap_or(false);
    let base_renamed_dir_exists = fs::try_exists(&base_renamed_dir).await.unwrap_or(false);
    let mono_dir_exists = fs::try_exists(&mono_dir).await.unwrap_or(false);

    if !base_dir_exists {
        return Err(MessageType::error(t!("error.file_locate")));
    }

    if mono_dir_exists {
        fs::rename(&base_dir, &base_renamed_dir)
            .await
            .inspect_err(|e| eprintln!("{e}"))
            .map_err(|_| MessageType::error(t!("error.file_rename")))?;

        fs::rename(&mono_dir, &base_dir)
            .await
            .inspect_err(|e| eprintln!("{e}"))
            .map_err(|_| MessageType::error(t!("error.file_rename")))?;
    } else if !base_renamed_dir_exists {
        let _ = tx.send(StatusType::Message(MessageType::warning(t!(
            "warning.unityplayer"
        ))));
    }

    Ok(())
}

async fn copy_patch_files(ctx: &DirectoryList) -> Result<(), MessageType> {
    let app_dir = ctx.app_dir.as_ref().unwrap();
    let game_dir = ctx.game_dir.as_ref().unwrap();

    fs::copy(
        app_dir.join("doorstop_config.ini"),
        game_dir.join("doorstop_config.ini"),
    )
    .await
    .inspect_err(|e| eprintln!("{e}"))
    .map_err(|_| MessageType::error(t!("error.file_copy")))?;

    fs::copy(app_dir.join("winhttp.dll"), game_dir.join("winhttp.dll"))
        .await
        .inspect_err(|e| eprintln!("{e}"))
        .map_err(|_| MessageType::error(t!("error.file_copy")))?;

    Ok(())
}

async fn copy_dir_all(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
) -> std::io::Result<()> {
    fs::create_dir_all(&dst).await?;
    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = &entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            std::boxed::Box::pin(copy_dir_all(
                entry.path(),
                dst.as_ref().join(entry.file_name()),
            ))
            .await?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name())).await?;
        }
    }
    Ok(())
}

async fn wait_for_process(name: &str, timeout: Duration) -> Result<(), Elapsed> {
    let mut sys = sysinfo::System::new();

    tokio::time::timeout(timeout, async {
        loop {
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::All,
                true,
                sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
            );

            if sys
                .processes()
                .values()
                .any(|p| p.exe().and_then(|path| path.file_name()) == Some(name.as_ref()))
            {
                break;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
}
