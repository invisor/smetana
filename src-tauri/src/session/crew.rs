//! The Smetana-owned tree for one driven Crew package.
//!
//! Provider ids are deliberately kept in the private index. The public node id
//! is minted here and is stable for a package's life, including after a child
//! has finished, so a Vue key never changes underneath a selected journal.

use std::collections::BTreeMap;

use crate::agents::crew::{ProviderNode, ProviderState};
use super::journal::Journal;
use super::model::{Event, EventKind};

/// The session worker owns one of these per Crew package. Its project is kept
/// beside the topology so `crew:tree` can update the right window without a
/// provider id becoming a front-end key.
pub struct CrewPackage {
    pub project: String,
    pub root: CrewNodeId,
    pub tree: CrewTree,
    /// One journal per stable Smetana node. It is deliberately not keyed by a
    /// provider id: a completed child keeps its own log until package cleanup.
    pub journals: BTreeMap<CrewNodeId, Journal>,
}

impl CrewPackage {
    /// The session worker allocates this id globally. A tree-local `1` for
    /// every package would replace another package in the worker map.
    pub fn new(project: String, root: CrewNodeId, label: impl Into<String>) -> Self {
        let mut tree = CrewTree::default();
        tree.root_with_id(root, label);
        let mut journals = BTreeMap::new();
        journals.insert(root, Journal::new());
        Self { project, root, tree, journals }
    }

    pub fn apply(&mut self, node: ProviderNode) -> Option<CrewNodeId> {
        let id = self.tree.upsert(self.root, node)?;
        self.journals.entry(id).or_insert_with(Journal::new);
        Some(id)
    }

    pub fn snapshot(&self, node: CrewNodeId) -> Option<(Vec<Event>, u64, CrewState)> {
        let journal = self.journals.get(&node)?;
        let state = self.tree.node(node)?.state;
        let (events, seq) = journal.snapshot();
        Some((events, seq, state))
    }

    pub fn append(&mut self, node: CrewNodeId, kinds: Vec<EventKind>) -> Option<Vec<Event>> {
        let journal = self.journals.get_mut(&node)?;
        let at = chrono::Utc::now().to_rfc3339();
        Some(kinds.into_iter().map(|kind| journal.append(kind, at.clone())).collect())
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
        self.root_with_id(id, label);
        id
    }

    /// Register a public root allocated by the session worker. Provider ids
    /// remain private, while the root namespaces the stable Vue-facing ids.
    pub fn root_with_id(&mut self, id: CrewNodeId, label: impl Into<String>) {
        self.next = self.next.max(id);
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

    /// Apply a status-only provider notification without replacing the label
    /// and parent learned from `thread/started`/`thread/list`.
    pub fn update_provider(
        &mut self,
        provider_id: &str,
        state: ProviderState,
        can_message: bool,
    ) -> bool {
        let Some(id) = self.provider.get(provider_id).copied() else { return false };
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.state = CrewState::from(state);
        node.can_message = can_message && !matches!(node.state, CrewState::Done | CrewState::Failed);
        true
    }

    pub fn fail_node(&mut self, id: CrewNodeId) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        node.state = CrewState::Failed;
        node.can_message = false;
        true
    }

    pub fn node(&self, id: CrewNodeId) -> Option<&CrewNode> {
        self.nodes.get(&id)
    }

    pub fn provider_id(&self, id: CrewNodeId) -> Option<&str> {
        self.provider
            .iter()
            .find_map(|(provider, node)| (*node == id).then_some(provider.as_str()))
    }

    pub fn node_for_label(&self, label: &str) -> Option<CrewNodeId> {
        self.nodes
            .values()
            .find(|node| node.label == label)
            .map(|node| node.id)
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

    #[test]
    fn worker_allocated_roots_do_not_collide_between_packages() {
        let first = CrewPackage::new("/one".into(), 41, "Lead");
        let second = CrewPackage::new("/two".into(), 42, "Lead");
        assert_ne!(first.root, second.root);
        assert!(first.tree.node(41).is_some());
        assert!(second.tree.node(42).is_some());
    }

    #[test]
    fn two_children_keep_separate_journals_under_stable_node_ids() {
        let mut package = CrewPackage::new("/project".into(), 7, "Lead");
        let first = package
            .apply(worker("one", None, ProviderState::Running))
            .expect("first node");
        let second = package
            .apply(worker("two", None, ProviderState::Running))
            .expect("second node");
        package.append(
            first,
            vec![super::EventKind::UserMessage {
                text: "ONLY-FIRST".into(),
                attachments: Vec::new(),
            }],
        );
        let (first_events, _, _) = package.snapshot(first).expect("first journal");
        let (second_events, _, _) = package.snapshot(second).expect("second journal");
        assert_eq!(first_events.len(), 1);
        assert!(second_events.is_empty());
    }

    #[test]
    fn a_child_failure_does_not_fail_its_sibling_or_root() {
        let mut package = CrewPackage::new("/project".into(), 7, "Lead");
        let first = package.apply(worker("one", None, ProviderState::Running)).unwrap();
        let second = package.apply(worker("two", None, ProviderState::Running)).unwrap();
        assert!(package.tree.fail_node(first));
        assert_eq!(package.tree.node(first).unwrap().state, CrewState::Failed);
        assert_eq!(package.tree.node(second).unwrap().state, CrewState::Running);
        assert_eq!(package.tree.node(package.root).unwrap().state, CrewState::Starting);
    }
}
