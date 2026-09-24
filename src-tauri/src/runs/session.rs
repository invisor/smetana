//! Transport selection frozen for one run package.
//!
//! The run loop must decide this before it claims work or creates a worktree:
//! changing from a driven tree to a PTY after the lead has started would leave
//! two incompatible views of one package and makes addressed child messages
//! impossible to reason about.

use crate::agents::crew::CrewCapabilities;

use super::model::RunMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Pty,
    DrivenCrew,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunSession {
    pub provider: String,
    pub transport: Transport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedCrew {
    pub provider: String,
    pub version: String,
    pub missing: &'static str,
}

impl std::fmt::Display for UnsupportedCrew {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} cannot run Crew in the conversation panel: {} is unavailable",
            self.provider, self.version, self.missing
        )
    }
}

/// The one selection point between the run loop and its transport. Auto and
/// Solo intentionally remain on their existing PTY route; a disabled panel
/// leaves supervised Crew there too.
pub fn select(
    mode: RunMode,
    conversation_panel: bool,
    provider: impl Into<String>,
    version: impl Into<String>,
    capabilities: CrewCapabilities,
) -> Result<RunSession, UnsupportedCrew> {
    let provider = provider.into();
    let version = version.into();
    if mode != RunMode::Supervised || !conversation_panel {
        return Ok(RunSession {
            provider,
            transport: Transport::Pty,
        });
    }
    let missing = if !capabilities.discover_nodes {
        Some("native agent discovery")
    } else if !capabilities.separate_journals {
        Some("separate agent journals")
    } else if !capabilities.addressed_messages {
        Some("addressed agent messages")
    } else {
        None
    };
    match missing {
        Some(missing) => Err(UnsupportedCrew {
            provider,
            version,
            missing,
        }),
        None => Ok(RunSession {
            provider,
            transport: Transport::DrivenCrew,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: CrewCapabilities = CrewCapabilities {
        discover_nodes: true,
        separate_journals: true,
        addressed_messages: true,
    };

    #[test]
    fn only_supervised_crew_with_the_panel_enabled_selects_a_driven_tree() {
        assert_eq!(
            select(RunMode::Supervised, true, "Codex", "0.155.1", ALL)
                .unwrap()
                .transport,
            Transport::DrivenCrew
        );
        assert_eq!(
            select(RunMode::Auto, true, "Codex", "0.155.1", ALL)
                .unwrap()
                .transport,
            Transport::Pty
        );
        assert_eq!(
            select(RunMode::Solo, true, "Claude Code", "2.1.281", ALL)
                .unwrap()
                .transport,
            Transport::Pty
        );
        assert_eq!(
            select(RunMode::Supervised, false, "Claude Code", "2.1.281", ALL)
                .unwrap()
                .transport,
            Transport::Pty
        );
    }

    #[test]
    fn an_unsupported_transport_refuses_before_the_run_can_claim_work() {
        let error = select(
            RunMode::Supervised,
            true,
            "Claude Code",
            "2.1.281",
            CrewCapabilities {
                addressed_messages: false,
                ..ALL
            },
        )
        .unwrap_err();
        assert_eq!(error.missing, "addressed agent messages");
        assert!(error.to_string().contains("2.1.281"));
    }
}
