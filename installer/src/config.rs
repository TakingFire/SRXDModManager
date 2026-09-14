use std::collections::HashSet;

use model::Manifest;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum FilterBy {
    #[default]
    All,
    Installed,
    Uninstalled,
    Updatable,
    Unrecognized,
}

#[derive(Debug, Default, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum SortBy {
    Recent,
    #[default]
    Title,
    Author,
}

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
pub enum PopupState {
    #[default]
    Active,
    Dismissed,
    Disabled,
}

impl PopupState {
    pub const fn enable(&mut self) {
        if matches!(self, Self::Dismissed) {
            *self = Self::Active;
        }
    }

    pub const fn dismiss(&mut self) {
        *self = Self::Dismissed;
    }

    pub const fn disable(&mut self) {
        *self = Self::Disabled;
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallerConfig {
    pub manifest: Manifest,

    pub popup_disclaimer: PopupState,
    pub popup_linux_guide: PopupState,
    pub popup_existing_config: PopupState,

    pub categories: HashSet<String>,
    pub filter_by: FilterBy,
    pub sort_by: SortBy,
    pub search: String,

    pub show_unrecognized_mods: bool,
    pub show_outdated_mods: bool,
    pub show_outdated_app: bool,

    pub show_game_console: bool,
    pub show_app_console: bool,
}

impl Default for InstallerConfig {
    fn default() -> Self {
        Self {
            manifest: Manifest::default(),

            popup_disclaimer: PopupState::Active,
            popup_linux_guide: PopupState::Active,
            popup_existing_config: PopupState::Dismissed,

            categories: HashSet::default(),
            filter_by: FilterBy::default(),
            sort_by: SortBy::default(),
            search: String::default(),

            show_unrecognized_mods: true,
            show_outdated_mods: true,
            show_outdated_app: true,

            show_game_console: true,
            show_app_console: false,
        }
    }
}
