use std::cell::RefCell;
use std::rc::Rc;

use model::Mod;

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

pub type ModEntryRef = Rc<RefCell<ModEntry>>;
