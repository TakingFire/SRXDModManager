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
    pub fn enable(&mut self) {
        if matches!(self, PopupState::Dismissed) {
            *self = PopupState::Active;
        }
    }

    pub fn dismiss(&mut self) {
        *self = PopupState::Dismissed;
    }

    pub fn disable(&mut self) {
        *self = PopupState::Disabled;
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
        }
    }
}
