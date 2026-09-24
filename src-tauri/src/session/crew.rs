//! The Smetana-owned tree for one driven Crew package.
//!
//! Provider ids are deliberately kept in the private index. The public node id
//! is minted here and is stable for a package's life, including after a child
//! has finished, so a Vue key never changes underneath a selected journal.

use std::collections::BTreeMap;

use crate::agents::crew::{ProviderNode, ProviderState};

/// The session worker owns one of these per Crew package. Its project is kept
/// beside the topology so `crew:tree` can update the right window without a
/// provider id becoming a front-end key.
pub struct CrewPackage {
    pub project: String,
    pub root: CrewNodeId,
    pub tree: CrewTree,
}

impl CrewPackage {
    pub fn new(project: String, label: impl Into<String>) -> Self {
        let mut tree = CrewTree::default();
        let root = tree.root(label);
        Self { project, root, tree }
    }
}

pub type CrewNodeId = u64;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrewNode {
    pub id: CrewNodeId,
    pub parent: Option<CrewNodeId>,
    pub root: CrewNodeId,
    pub state: CrewState,
    pub label: String,
    pub can_message: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrewState {
    Starting,
    Running,
    Waiting,
    Done,
    Failed,
}

impl From<ProviderState> for CrewState {
    fn from(value: ProviderState) -> Self {
        match value {
            ProviderState::Starting => Self::Starting,
            ProviderState::Running => Self::Running,
            ProviderState::Waiting => Self::Waiting,
            ProviderState::Done => Self::Done,
            ProviderState::Failed => Self::Failed,
        }
    }
}

/// One package's tree and the provider-id index it needs to reconcile updates.
/// The index never leaves this module; no front-end caller can accidentally use
/// it as a key or route an addressed message by an untrusted provider id.
#[derive(Default)]
pub struct CrewTree {
    next: CrewNodeId,
    nodes: BTreeMap<CrewNodeId, CrewNode>,
    provider: BTreeMap<String, CrewNodeId>,
}

impl CrewTree {
    pub fn root(&mut self, label: impl Into<String>) -> CrewNodeId {
        let id = self.mint();
        self.nodes.insert(
            id,
            CrewNode {
                id,
                parent: None,
                root: id,
                state: CrewState::Starting,
                label: label.into(),
                can_message: true,
            },
        );
        id
    }

    /// Bind a provider lead identity only after its structured runtime has
    /// supplied one. This avoids guessing a Claude team name from a CLI
    /// session id while still letting a Codex child's `parentThreadId` resolve
    /// to the Smetana root immediately.
    pub fn bind_root(&mut self, root: CrewNodeId, provider_id: impl Into<String>) -> bool {
        if !self.nodes.contains_key(&root) {
            return false;
        }
        self.provider.insert(provider_id.into(), root);
        true
    }

    /// Applies one provider update. An unknown parent is intentionally attached
    /// to `root`: provider notifications can arrive out of order, and making a
    /// child disappear is worse than temporarily drawing it one level too high.
    /// A later parent update reparents it through the same stable Smetana id.
    pub fn upsert(&mut self, root: CrewNodeId, incoming: ProviderNode) -> Option<CrewNodeId> {
        if !self.nodes.contains_key(&root) {
            return None;
        }
        let id = self.provider.get(&incoming.id).copied().unwrap_or_else(|| {
            let id = self.mint();
            self.provider.insert(incoming.id.clone(), id);
            id
        });
        let parent = incoming
            .parent
            .as_deref()
            .and_then(|provider| self.provider.get(provider).copied())
            .filter(|parent| *parent != id)
            .or(Some(root));
        let state = CrewState::from(incoming.state);
        let label = incoming.label.unwrap_or_else(|| "Background agent".into());
        self.nodes.insert(
            id,
            CrewNode {
                id,
                parent,
                root,
                state,
                label,
                can_message: incoming.can_message
                    && !matches!(state, CrewState::Done | CrewState::Failed),
            },
        );
        Some(id)
    }

    pub fn node(&self, id: CrewNodeId) -> Option<&CrewNode> {
        self.nodes.get(&id)
    }

    pub fn nodes(&self) -> Vec<CrewNode> {
        self.nodes.values().cloned().collect()
    }

    /// Package cleanup is all-or-nothing. Finished children remain available
    /// while the root is alive; only this operation drops their journals and
    /// indexes together, preventing orphan rows after Stop.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.provider.clear();
    }

    fn mint(&mut self) -> CrewNodeId {
        self.next = self.next.saturating_add(1);
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker(id: &str, parent: Option<&str>, state: ProviderState) -> ProviderNode {
        ProviderNode {
            id: id.into(),
            parent: parent.map(str::to_owned),
            state,
            label: Some(id.into()),
            can_message: true,
        }
    }

    #[test]
    fn provider_ids_never_become_public_node_ids() {
        let mut tree = CrewTree::default();
        let root = tree.root("Lead");
        let child = tree
            .upsert(root, worker("provider-child", None, ProviderState::Running))
            .unwrap();
        assert_ne!(child.to_string(), "provider-child");
        assert_eq!(tree.node(child).unwrap().root, root);
    }

    #[test]
    fn a_provider_child_of_the_lead_resolves_to_the_existing_root() {
        let mut tree = CrewTree::default();
        let root = tree.root("Lead");
        assert!(tree.bind_root(root, "lead-thread"));
        let child = tree
            .upsert(
                root,
                worker("child-thread", Some("lead-thread"), ProviderState::Running),
            )
            .unwrap();
        assert_eq!(tree.node(child).unwrap().parent, Some(root));
    }

    #[test]
    fn nodes_keep_their_id_and_finished_journal_address_until_package_cleanup() {
        let mut tree = CrewTree::default();
        let root = tree.root("Lead");
        let child = tree
            .upsert(root, worker("child", None, ProviderState::Running))
            .unwrap();
        assert_eq!(tree.node(child).unwrap().state, CrewState::Running);
        assert!(tree.node(child).unwrap().can_message);
        assert_eq!(
            tree.upsert(root, worker("child", None, ProviderState::Done)),
            Some(child)
        );
        assert_eq!(tree.node(child).unwrap().state, CrewState::Done);
        assert!(!tree.node(child).unwrap().can_message);
        tree.clear();
        assert!(tree.nodes().is_empty());
    }

    #[test]
    fn an_out_of_order_child_is_visible_then_reparented_without_a_new_key() {
        let mut tree = CrewTree::default();
        let root = tree.root("Lead");
        let child = tree
            .upsert(
                root,
                worker("child", Some("parent"), ProviderState::Running),
            )
            .unwrap();
        assert_eq!(tree.node(child).unwrap().parent, Some(root));
        let parent = tree
            .upsert(root, worker("parent", None, ProviderState::Running))
            .unwrap();
        assert_eq!(
            tree.upsert(
                root,
                worker("child", Some("parent"), ProviderState::Running)
            ),
            Some(child)
        );
        assert_eq!(tree.node(child).unwrap().parent, Some(parent));
    }
}
