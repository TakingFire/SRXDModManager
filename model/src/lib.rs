use serde::{Deserialize, Serialize};

pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Manifest {
    #[serde(default = "get_version")]
    pub version: String,
    pub mods: Vec<Mod>,
    pub categories: Vec<String>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: get_version(),
            mods: Default::default(),
            categories: Default::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Mod {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub repository: String,
    pub url: String,
    pub file: String,
    pub categories: Vec<String>,
    pub versions: Vec<Version>,
    pub dependencies: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Version {
    pub name: String,
    pub created_at: String,
    pub url: String,
    pub digest: String,
}
