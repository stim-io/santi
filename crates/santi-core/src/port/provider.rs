use futures::Stream;
use serde_json::Value;

use crate::{error::Result, provider::ProviderInputMessage};

#[derive(Clone, Debug)]
pub struct ProviderRequest {
    pub model: String,
    pub instructions: Option<String>,
    pub input: Vec<ProviderInputMessage>,
    pub tools: Option<Vec<Value>>,
    pub previous_response_id: Option<String>,
    pub function_call_output: Option<FunctionCallOutput>,
}

#[derive(Clone, Debug)]
pub struct FunctionCallOutput {
    pub call_id: String,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProviderEvent {
    OutputTextDelta(String),
    Completed { response_id: Option<String> },
}

pub trait Provider {
    type EventStream: Stream<Item = Result<ProviderEvent>> + Send;

    fn stream(&self, request: ProviderRequest) -> Self::EventStream;
}
