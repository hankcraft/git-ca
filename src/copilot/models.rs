use serde::Deserialize;

use super::client::{map_error, Client};
use crate::error::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub capabilities: Option<Capabilities>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Capabilities {
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResp {
    data: Vec<Model>,
}

impl Client {
    pub async fn list_models(&self) -> Result<Vec<Model>> {
        let url = format!("{}/models", self.base_url());
        let resp = self.http().get(url).headers(self.headers()).send().await?;
        if !resp.status().is_success() {
            return Err(map_error(resp).await);
        }
        let parsed: ModelsResp = resp.json().await?;
        Ok(parsed.data)
    }

    pub async fn list_chat_models(&self) -> Result<Vec<Model>> {
        let all = self.list_models().await?;
        Ok(all
            .into_iter()
            .filter(|m| {
                m.capabilities
                    .as_ref()
                    .and_then(|c| c.kind.as_deref())
                    .map(|k| k == "chat")
                    .unwrap_or(true)
            })
            .collect())
    }
}
