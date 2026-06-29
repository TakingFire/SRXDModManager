use model::Mod;
use serde::Deserialize;

use crate::providers::{ProviderType, github};

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Template {
    pub mods: Vec<ModTemplate>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct ModTemplate {
    pub name: String,
    pub author: String,
    pub description: String,
    pub repository: String,
    pub provider: ProviderType,
    pub file: String,
    pub categories: Vec<String>,
    pub dependencies: Vec<String>,
}

impl From<&ModTemplate> for Mod {
    fn from(value: &ModTemplate) -> Self {
        Self {
            id: [value.author.clone(), value.name.clone()]
                .join("_")
                .replace(char::is_whitespace, ""),
            name: value.name.clone(),
            author: value.author.clone(),
            description: value.description.clone(),
            url: value.repository.clone(),
            file: value.file.clone(),
            categories: value.categories.clone(),
            dependencies: value.dependencies.clone(),
            ..Default::default()
        }
    }
}

const TEMPLATE_URL: &str = "https://raw.githubusercontent.com/TakingFire/SRXDModManager/refs/heads/main/server/mods/template.toml";

pub async fn get_template_local() -> anyhow::Result<Template> {
    let template: Template =
        toml::from_str(&tokio::fs::read_to_string("mods/template.toml").await?)?;

    Ok(template)
}

pub async fn get_template_github() -> anyhow::Result<Template> {
    let template: Template = toml::from_str(
        &github::CLIENT
            .get(TEMPLATE_URL)
            .send()
            .await?
            .text()
            .await?,
    )?;

    Ok(template)
}
