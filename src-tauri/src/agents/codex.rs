//! Codex.

use portable_pty::CommandBuilder;

use super::{Launch, Profile, SkillDelivery};

pub struct Codex;

impl Profile for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn binary(&self) -> &'static str {
        "codex"
    }
    fn delivery(&self) -> SkillDelivery {
        SkillDelivery::Inline
    }
    fn command(&self, _launch: &Launch) -> CommandBuilder {
        CommandBuilder::new(self.binary())
    }
}
