use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SessionOutputTextDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub delta: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionCompletedEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
}

#[derive(Clone, Debug)]
pub enum SessionStreamEvent {
    OutputTextDelta(String),
    Completed,
}
