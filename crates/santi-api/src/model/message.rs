#[derive(Clone, Debug)]
pub struct Message {
    pub id: String,
    pub r#type: String,
    pub role: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ToolCallArtifact {
    pub v: u8,
    pub tool_call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ToolResultArtifact {
    pub v: u8,
    pub tool_call_id: String,
    pub name: String,
    pub ok: bool,
    pub output: serde_json::Value,
}
