//! Claude Code.

use portable_pty::CommandBuilder;

use super::{Launch, Profile, SkillDelivery};

pub struct Claude;

impl Profile for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn binary(&self) -> &'static str {
        "claude"
    }
    fn delivery(&self) -> SkillDelivery {
        SkillDelivery::PluginDir
    }
    fn command(&self, _launch: &Launch) -> CommandBuilder {
        CommandBuilder::new(self.binary())
    }
}
