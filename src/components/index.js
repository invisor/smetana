/* The component library, as imported by product code. */

// core
export { default as Button } from './core/Button.vue'
export { default as Checkbox } from './core/Checkbox.vue'
export { default as EmptyState } from './core/EmptyState.vue'
export { default as Icon } from './core/Icon.vue'
export { default as IconButton } from './core/IconButton.vue'
export { default as Input } from './core/Input.vue'
export { default as Dropdown } from './core/Dropdown.vue'
export { default as Select } from './core/Select.vue'
export { default as Skeleton } from './core/Skeleton.vue'
export { default as Switch } from './core/Switch.vue'
export { default as Textarea } from './core/Textarea.vue'
export { default as Tooltip } from './core/Tooltip.vue'
export { iconNodes } from './core/icons.js'
export { useInteractive } from './core/interactive.js'

// status
export { default as DependencyBand } from './status/DependencyBand.vue'
export { default as DependencyMark } from './status/DependencyMark.vue'
export { default as DependencySpine } from './status/DependencySpine.vue'
export { default as StatusBadge } from './status/StatusBadge.vue'
export { default as StatusDot } from './status/StatusDot.vue'
export {
  RESERVED,
  STATUS_GLYPH,
  attentionLevel,
  hashStatus,
  normalizeStatus,
  statusCode,
  statusColors,
  statusSlot
} from './status/status.js'

// shell
export { default as AppShell } from './shell/AppShell.vue'
export { default as CommandPalette } from './shell/CommandPalette.vue'
export { default as Panel } from './shell/Panel.vue'
export { default as ProjectRail } from './shell/ProjectRail.vue'
export { default as ProjectTile } from './shell/ProjectTile.vue'
export { default as Resizer } from './shell/Resizer.vue'
export { default as ScopeIndicator } from './shell/ScopeIndicator.vue'
export { default as Tab } from './shell/Tab.vue'
export { default as TabBar } from './shell/TabBar.vue'
export { default as TaskSearchButton } from './shell/TaskSearchButton.vue'
export { NEW_TAB_ITEMS } from './shell/newTabMenu.js'

// kanban
export { default as Assignee } from './kanban/Assignee.vue'
export { default as AttachmentStrip } from './kanban/AttachmentStrip.vue'
export { default as ColumnHeader } from './kanban/ColumnHeader.vue'
export { default as DraftInspector } from './kanban/DraftInspector.vue'
export { default as KanbanBoard } from './kanban/KanbanBoard.vue'
export { default as KanbanColumn } from './kanban/KanbanColumn.vue'
export { default as Markdown } from './kanban/Markdown.vue'
export { default as MarkdownInline } from './kanban/MarkdownInline.vue'
export { default as NewTaskModal } from './kanban/NewTaskModal.vue'
export { default as PromoteColumnModal } from './kanban/PromoteColumnModal.vue'
export { default as TaskCard } from './kanban/TaskCard.vue'
export { default as TaskInspector } from './kanban/TaskInspector.vue'
export { default as TypeBadge } from './kanban/TypeBadge.vue'

// agent
export { default as AgentList } from './agent/AgentList.vue'
export { default as AnsiText } from './agent/AnsiText.vue'
export { default as ChatMessage } from './agent/ChatMessage.vue'
export { default as ClaimedTasks } from './agent/ClaimedTasks.vue'
export { default as CodeBlock } from './agent/CodeBlock.vue'
export { default as LogLine } from './agent/LogLine.vue'
export { default as LogToolbar } from './agent/LogToolbar.vue'
export { default as LogView } from './agent/LogView.vue'
export { default as ToolCall } from './agent/ToolCall.vue'
export { parseAnsi } from './agent/ansi.js'

// files
export { default as DiffView } from './files/editor/DiffView.vue'
export { default as FileEditor } from './files/FileEditor.vue'
export { default as FileTree } from './files/FileTree.vue'
export { default as FileTreeDraftRow } from './files/FileTreeDraftRow.vue'
export { default as FileTreeRow } from './files/FileTreeRow.vue'

// git
export { default as BranchList } from './git/BranchList.vue'
export { default as ChangeList } from './git/ChangeList.vue'
export { default as CommitBox } from './git/CommitBox.vue'
export { default as ConflictModal } from './git/ConflictModal.vue'
export { default as GitPanel } from './git/GitPanel.vue'
export { default as NewBranchModal } from './git/NewBranchModal.vue'
export { default as RepoList } from './git/RepoList.vue'
export { default as SectionHeader } from './git/SectionHeader.vue'
export { branchMenuItems } from './git/branchMenu.js'
export { branchNameError, canCreate } from './git/branchName.js'
export { branchRows, currentChain, expandedFolders, toggleFolder } from './git/branchTree.js'
export { changeStatus } from './git/changeStatus.js'
export { canCommit, canSuggest, commitHint, commitLabel } from './git/commitBox.js'
export { gitActions } from './git/gitActions.js'
export { BRANCH_ROWS, filler, resolveDrag } from './git/sectionHeights.js'
export { AHEAD_TOKEN, BEHIND_TOKEN, folderBehind, pullAction, pushAction, trackingMark } from './git/tracking.js'

// notifications
export { default as NotificationCard } from './notifications/NotificationCard.vue'
export { default as NotificationPanel } from './notifications/NotificationPanel.vue'
export {
  MIB,
  THRESHOLDS_MIB,
  crossedThreshold,
  reachedThreshold,
  rememberAfter,
  stillOver,
  storageNotification
} from './notifications/notifications.js'

// overlays
export { default as ContextMenu } from './overlays/ContextMenu.vue'
export { default as ImageViewer } from './overlays/ImageViewer.vue'
export { default as MenuButton } from './overlays/MenuButton.vue'
export { default as Modal } from './overlays/Modal.vue'
export { default as PointerMenu } from './overlays/PointerMenu.vue'
export { default as Toast } from './overlays/Toast.vue'

// run
export { default as BranchSelect } from './run/BranchSelect.vue'
export { default as ReportView } from './run/ReportView.vue'
export { default as RunBar } from './run/RunBar.vue'
export { default as RunModal } from './run/RunModal.vue'
export { default as SetupProjectModal } from './run/SetupProjectModal.vue'

// settings
export { default as AboutSettings } from './settings/AboutSettings.vue'
export { default as AgentSettings } from './settings/AgentSettings.vue'
export { default as EditorSettings } from './settings/EditorSettings.vue'
export { default as GeneralSettings } from './settings/GeneralSettings.vue'
export { default as KanbanSettings } from './settings/KanbanSettings.vue'
export { default as SettingsRow } from './settings/SettingsRow.vue'
export { default as StorageSettings } from './settings/StorageSettings.vue'

// terminal
export { default as TerminalView } from './terminal/TerminalView.vue'
