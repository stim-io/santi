use crate::config::Mode;

#[derive(Clone)]
pub struct ApiCapabilities {
    pub health: bool,
    pub sessions: bool,
    pub soul: bool,
    pub admin_hooks: bool,
    pub streaming: bool,
}

pub fn default_capabilities(mode: &Mode) -> ApiCapabilities {
    match mode {
        Mode::Distributed => ApiCapabilities {
            health: true,
            sessions: true,
            soul: true,
            admin_hooks: true,
            streaming: true,
        },
        Mode::Standalone => ApiCapabilities {
            health: true,
            sessions: true,
            soul: true,
            admin_hooks: true,
            streaming: false,
        },
    }
}
