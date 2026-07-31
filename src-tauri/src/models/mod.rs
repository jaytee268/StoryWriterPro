use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneInput {
    pub id: String,
    pub chapter_id: String,
    pub title: String,
    pub content: String,
    pub pov: String,
    pub location: String,
    pub story_time: String,
    pub status: String,
    pub goal: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryEntityInput {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: String,
    pub status: String,
    pub confidence: f64,
    pub source: String,
    pub chapter: String,
    pub scene: String,
    pub author_confirmed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub available: bool,
    pub label: String,
    pub detail: String,
}
