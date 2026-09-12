use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

use flume::{Receiver, Sender};
use model::{Manifest, Mod, Version};

use crate::patch::{self, MessageType, StatusType};

#[derive(Debug, Default, Clone)]
pub enum ModEntryState {
    #[default]
    Uninstalled,
    PendingUninstall,
    Installed,
    PendingInstall,
    PendingVersionChangeFrom(usize),
}

#[derive(Debug, Default, Clone)]
pub struct ModEntry {
    pub entry: Mod,
    pub state: ModEntryState,
    pub selected_version: usize,
    pub active_dependents: usize,
    pub recognized: bool,
}

pub type ModEntryRef = Rc<RefCell<ModEntry>>;

#[derive(Default, Clone)]
pub struct DirectoryList {
    pub app_dir: Option<PathBuf>,
    pub game_dir: Option<PathBuf>,
    pub steam_dir: Option<PathBuf>,
}

impl ModEntry {
    pub fn set_version(&mut self, version: usize) {
        if version == self.selected_version {
            return;
        }

        match self.state {
            ModEntryState::Installed => {
                self.state = ModEntryState::PendingVersionChangeFrom(self.selected_version);
            }
            ModEntryState::PendingVersionChangeFrom(prev) if version == prev => {
                self.state = ModEntryState::Installed;
            }

            _ => {}
        }

        self.selected_version = version;
    }
}

#[derive(Debug, Default)]
pub enum InstallerState {
    #[default]
    Init,
    Ready,
    Launching,
    Outdated,
    Error,
}

pub struct Installer {
    pub state: InstallerState,
    pub tx: Sender<StatusType>,
    pub rx: Receiver<StatusType>,
    pub dirs: DirectoryList,
    pub mods: Vec<ModEntryRef>,
    pub manifest: Manifest,
    pub id_map: HashMap<String, ModEntryRef>,
    pub digest_map: HashMap<String, (ModEntryRef, usize)>,
    pub log: Vec<MessageType>,

    pub force_ui_update: bool,
    pub show_config_popup: bool,
}

impl Default for Installer {
    fn default() -> Self {
        let (tx, rx) = flume::unbounded();

        Self {
            state: InstallerState::default(),
            tx,
            rx,
            dirs: DirectoryList::default(),
            mods: Vec::default(),
            manifest: Manifest::default(),
            id_map: HashMap::default(),
            digest_map: HashMap::default(),
            log: Vec::default(),

            force_ui_update: false,
            show_config_popup: false,
        }
    }
}

impl Installer {
    pub fn init(&mut self) {
        self.state = InstallerState::Init;
        self.log.clear();
        self.log.push(MessageType::default(t!("status.starting")));
        self.get_directories();
    }

    pub fn update(&mut self) {
        while let Ok(status) = self.rx.try_recv() {
            match status {
                StatusType::Message(msg) => self.log(msg),

                StatusType::Success(ctx) => match ctx {
                    patch::TaskContext::GetDirectories(ctx) => {
                        self.dirs = ctx;
                        self.get_manifest();
                    }

                    patch::TaskContext::GetManifest(ctx) => {
                        self.manifest = ctx.out_manifest;
                        self.build_mod_list();
                        self.build_id_digest_maps();

                        if ctx.out_outdated {
                            self.state = InstallerState::Outdated;
                        } else {
                            self.get_patcher();
                        }
                    }

                    patch::TaskContext::GetPatcher(_) => {
                        self.get_existing_config();
                        self.get_installed_mods();
                    }

                    patch::TaskContext::GetInstalledMods(ctx) => {
                        let mut installed_count = 0;

                        for digest in &ctx.out_digest_list {
                            if let Some((entry_ref, installed_version)) =
                                self.digest_map.get(digest)
                            {
                                let mut entry = entry_ref.borrow_mut();

                                entry.state = ModEntryState::Installed;
                                entry.selected_version = *installed_version;
                                entry.set_version(0);
                                installed_count += 1;

                                for dependency in self.get_dependencies(&entry) {
                                    dependency.borrow_mut().active_dependents += 1;
                                }
                            } else {
                                let entry = ModEntry {
                                    recognized: false,
                                    entry: Mod {
                                        id: digest.into(),
                                        name: digest.into(),
                                        author: t!("modentry.unknown").into(),
                                        versions: vec![Version {
                                            digest: digest.into(),
                                            ..Default::default()
                                        }],
                                        ..Default::default()
                                    },
                                    state: ModEntryState::Installed,
                                    selected_version: 0,
                                    ..Default::default()
                                };

                                let entry_ref = Rc::new(RefCell::new(entry));

                                self.mods.push(entry_ref.clone());
                                self.digest_map.insert(digest.into(), (entry_ref, 0));
                            }
                        }

                        self.log(MessageType::success(t!(
                            "status.mods_count",
                            count = installed_count
                        )));

                        let unrecognized_count = ctx.out_digest_list.len() - installed_count;

                        if unrecognized_count > 0 {
                            self.log(MessageType::warning(t!(
                                "warning.mod_unrecognized",
                                count = unrecognized_count
                            )));
                        }

                        self.state = InstallerState::Ready;
                        self.log(MessageType::success(t!("status.ready")));
                    }

                    patch::TaskContext::GetExistingConfig(_) => {
                        self.log(MessageType::default(t!("status.config_found")));
                        self.show_config_popup = true;
                    }

                    patch::TaskContext::CopyExistingConfig(_) => {
                        self.log(MessageType::success(t!("status.config_copied")));
                    }

                    patch::TaskContext::InstallMod(ctx) => {
                        if let Some(entry) = self.get_entry_ref(&ctx.entry) {
                            entry.borrow_mut().state = ModEntryState::Installed;

                            self.log(MessageType::success(t!(
                                "status.mod_install",
                                name = ctx.entry.entry.id
                            )));
                        }
                    }

                    patch::TaskContext::UninstallMod(ctx) => {
                        if let Some(entry) = self.get_entry_ref(&ctx.entry) {
                            entry.borrow_mut().state = ModEntryState::Uninstalled;

                            if !entry.borrow().recognized
                                && let Some(idx) =
                                    self.mods.iter().position(|e| Rc::ptr_eq(e, &entry))
                            {
                                self.mods.remove(idx);
                            }

                            self.log(MessageType::success(t!(
                                "status.mod_uninstall",
                                name = ctx.entry.entry.id
                            )));
                        }
                    }

                    patch::TaskContext::PatchGameFiles(_)
                    | patch::TaskContext::UnpatchGameFiles(_) => {
                        self.launch_game();
                    }

                    patch::TaskContext::LaunchGame(_) => {
                        self.state = InstallerState::Ready;
                    }
                },

                StatusType::Error(ctx) => match ctx {
                    patch::TaskContext::GetManifest(ctx) => {
                        if ctx.out_outdated {
                            self.state = InstallerState::Outdated;
                        } else {
                            self.state = InstallerState::Error;
                        }
                    }

                    patch::TaskContext::InstallMod(ctx) => {
                        if let Some(entry) = self.get_entry_ref(&ctx.entry) {
                            entry.borrow_mut().state = ModEntryState::Uninstalled;
                        }
                    }

                    patch::TaskContext::UninstallMod(ctx) => {
                        if let Some(entry) = self.get_entry_ref(&ctx.entry) {
                            entry.borrow_mut().state = ModEntryState::Installed;
                        }
                    }

                    patch::TaskContext::GetExistingConfig(_)
                    | patch::TaskContext::CopyExistingConfig(_) => {}

                    _ => self.state = InstallerState::Error,
                },
            }

            self.force_ui_update = true;
        }
    }

    pub fn get_entry_ref(&self, entry: &ModEntry) -> Option<ModEntryRef> {
        Some(
            self.digest_map
                .get(&entry.entry.versions[entry.selected_version].digest)?
                .0
                .clone(),
        )
    }

    pub fn get_dependencies(&self, entry: &ModEntry) -> Vec<ModEntryRef> {
        entry
            .entry
            .dependencies
            .iter()
            .filter_map(|id| self.id_map.get(id))
            .cloned()
            .collect()
    }

    pub fn log(&mut self, msg: MessageType) {
        self.log.push(msg);
        self.force_ui_update = true;
    }

    pub fn build_id_digest_maps(&mut self) {
        for entry_ref in &self.mods {
            let entry = &entry_ref.borrow().entry;
            self.id_map.insert(entry.id.clone(), entry_ref.clone());
            for (i, version) in entry.versions.iter().enumerate() {
                self.digest_map
                    .insert(version.digest.clone(), (entry_ref.clone(), i));
            }
        }
    }

    pub fn build_mod_list(&mut self) {
        for entry in &self.manifest.mods {
            self.mods.push(Rc::new(RefCell::new(ModEntry {
                entry: entry.clone(),
                selected_version: 0,
                recognized: true,
                ..Default::default()
            })));
        }

        self.log(MessageType::success(t!(
            "status.mods_loaded",
            count = self.mods.len()
        )));
    }

    pub fn get_directories(&self) {
        patch::get_directories(DirectoryList::default(), self.tx.clone());
    }

    pub fn get_patcher(&self) {
        patch::get_patcher(self.dirs.clone(), self.tx.clone());
    }

    pub fn get_manifest(&self) {
        patch::get_manifest(patch::GetManifestContext::default(), self.tx.clone());
    }

    pub fn get_installed_mods(&self) {
        patch::get_installed_mods(
            patch::GetInstalledModsContext {
                directories: self.dirs.clone(),
                out_digest_list: Vec::new(),
            },
            self.tx.clone(),
        );
    }

    pub fn install_mod(&self, entry: &mut ModEntry) {
        if matches!(
            entry.state,
            ModEntryState::Installed | ModEntryState::PendingInstall
        ) {
            return;
        }

        entry.state = ModEntryState::PendingInstall; // moved from GUI

        patch::install_mod(
            patch::InstallModContext {
                directories: self.dirs.clone(),
                entry: entry.clone(),
            },
            self.tx.clone(),
        );

        for dependency in self.get_dependencies(entry) {
            dependency.borrow_mut().active_dependents += 1;
            let mut dep = dependency.borrow_mut();
            self.install_mod(&mut dep);
        }
    }

    pub fn uninstall_mod(&mut self, entry: &mut ModEntry) {
        if entry.active_dependents > 0 {
            self.log(MessageType::warning(t!(
                "warning.mod_required",
                count = entry.active_dependents
            )));
            return;
        }
        if matches!(
            entry.state,
            ModEntryState::Uninstalled | ModEntryState::PendingUninstall
        ) {
            return;
        }

        if !matches!(entry.state, ModEntryState::PendingVersionChangeFrom(_)) {
            entry.state = ModEntryState::PendingUninstall;
        }

        patch::uninstall_mod(
            patch::InstallModContext {
                directories: self.dirs.clone(),
                entry: entry.clone(),
            },
            self.tx.clone(),
        );

        for dependency in self.get_dependencies(entry) {
            let _remaining = {
                let mut dep = dependency.borrow_mut();
                dep.active_dependents = dep.active_dependents.saturating_sub(1);
                dep.active_dependents
            };
            // if remaining == 0 {
            //     let mut dep = dependency.borrow_mut();
            //     self.uninstall_mod(&mut dep);
            // }
        }
    }

    pub fn update_mod(&mut self, entry: &mut ModEntry) {
        self.uninstall_mod(entry);
        self.install_mod(entry);
    }

    pub fn get_existing_config(&self) {
        patch::get_existing_config(self.dirs.clone(), self.tx.clone());
    }

    pub fn copy_existing_config(&self) {
        patch::copy_existing_config(self.dirs.clone(), self.tx.clone());
    }

    pub fn patch_game_files(&mut self) {
        self.state = InstallerState::Launching;

        patch::patch_game_files(self.dirs.clone(), self.tx.clone());
    }

    pub fn unpatch_game_files(&mut self) {
        self.state = InstallerState::Launching;

        patch::unpatch_game_files(self.dirs.clone(), self.tx.clone());
    }

    pub fn launch_game(&mut self) {
        self.state = InstallerState::Launching;

        patch::launch_game(self.dirs.clone(), self.tx.clone());
    }
}
