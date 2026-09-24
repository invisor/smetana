//! Provider-neutral Crew topology events.
//!
//! A Crew lead is not a terminal screen and neither are its workers. This file
//! is deliberately about the structured records providers publish while they
//! create, update and finish native agents. The session worker owns the stable
//! Smetana ids and journals; adapters only translate provider vocabulary.

/// A provider-owned identity. It must never cross into a Vue key: providers may
/// recycle, omit or change it while a run is alive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderNode {
    pub id: String,
    pub parent: Option<String>,
    pub state: ProviderState,
    pub label: Option<String>,
    pub can_message: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderState {
    Starting,
    Running,
    Waiting,
    Done,
    Failed,
}

/// The three facts a run transport needs from a native Crew runtime. A caller
/// that cannot establish all three must reject the run before it can claim work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrewCapabilities {
    pub discover_nodes: bool,
    pub separate_journals: bool,
    pub addressed_messages: bool,
}

impl CrewCapabilities {
    pub const fn supported(self) -> bool {
        self.discover_nodes && self.separate_journals && self.addressed_messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transport_is_only_usable_when_all_three_crew_operations_exist() {
        assert!(CrewCapabilities {
            discover_nodes: true,
            separate_journals: true,
            addressed_messages: true
        }
        .supported());
        assert!(!CrewCapabilities {
            discover_nodes: true,
            separate_journals: true,
            addressed_messages: false
        }
        .supported());
    }
}
