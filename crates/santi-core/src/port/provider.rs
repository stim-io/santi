use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;

use crate::{error::Result, provider::ProviderInputMessage};

#[derive(Clone, Debug)]
pub struct ProviderRequest {
    pub model: String,
    pub instructions: Option<String>,
    pub input: Vec<ProviderInputMessage>,
    pub tools: Option<Vec<ProviderTool>>,
    pub previous_response_id: Option<String>,
    pub function_call_outputs: Option<Vec<FunctionCallOutput>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderTool {
    Function(ProviderFunctionTool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFunctionTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Clone, Debug)]
pub struct FunctionCallOutput {
    pub call_id: String,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFunctionCall {
    pub response_id: String,
    pub item_id: Option<String>,
    pub call_id: String,
    pub name: String,
    pub arguments_raw: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProviderEvent {
    OutputTextDelta(String),
    FunctionCallRequested(ProviderFunctionCall),
    Completed { response_id: Option<String> },
}

pub trait Provider: Send + Sync {
    fn stream(
        &self,
        request: ProviderRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<ProviderEvent>> + Send>>;
}
