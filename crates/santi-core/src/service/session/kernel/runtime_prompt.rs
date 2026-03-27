#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePrompt {
    pub meta: RuntimePromptMeta,
    pub sections: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePromptMeta {
    pub session_id: Option<String>,
    pub soul_id: Option<String>,
    pub has_soul_memory: bool,
    pub has_session_memory: bool,
    pub has_request_instructions: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimePromptSource {
    pub session_id: Option<String>,
    pub soul_id: Option<String>,
    pub soul_memory: Option<String>,
    pub session_memory: Option<String>,
    pub request_instructions: Option<String>,
}

pub fn build_runtime_prompt(source: RuntimePromptSource) -> RuntimePrompt {
    let mut sections = Vec::new();

    let soul_memory = source.soul_memory.filter(|text| !text.trim().is_empty());
    let session_memory = source.session_memory.filter(|text| !text.trim().is_empty());
    let request_instructions = source
        .request_instructions
        .filter(|text| !text.trim().is_empty());

    if let Some(text) = soul_memory.clone() {
        sections.push(text);
    }
    if let Some(text) = session_memory.clone() {
        sections.push(text);
    }
    if let Some(text) = request_instructions.clone() {
        sections.push(text);
    }

    RuntimePrompt {
        meta: RuntimePromptMeta {
            session_id: source.session_id,
            soul_id: source.soul_id,
            has_soul_memory: soul_memory.is_some(),
            has_session_memory: session_memory.is_some(),
            has_request_instructions: request_instructions.is_some(),
        },
        sections,
    }
}

impl RuntimePrompt {
    pub fn render(&self) -> Option<String> {
        let mut parts = Vec::new();

        parts.push("You are santi, a customized personal agent service.".to_string());
        parts.extend(self.sections.iter().cloned());

        let mut meta_lines = Vec::new();
        if let Some(session_id) = self.meta.session_id.as_deref() {
            meta_lines.push(format!("session_id: {}", session_id));
        }
        if let Some(soul_id) = self.meta.soul_id.as_deref() {
            meta_lines.push(format!("soul_id: {}", soul_id));
        }
        meta_lines.push(format!("has_soul_memory: {}", self.meta.has_soul_memory));
        meta_lines.push(format!(
            "has_session_memory: {}",
            self.meta.has_session_memory
        ));
        meta_lines.push(format!(
            "has_request_instructions: {}",
            self.meta.has_request_instructions
        ));

        if !meta_lines.is_empty() {
            parts.push(format!(
                "<santi-meta>\n{}\n</santi-meta>",
                meta_lines.join("\n")
            ));
        }

        let rendered = parts
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        if rendered.trim().is_empty() {
            None
        } else {
            Some(rendered)
        }
    }
}
