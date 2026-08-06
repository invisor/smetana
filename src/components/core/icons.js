/* The glyph vocabulary, and the only place Lucide is named.
   Registering icons explicitly is what keeps the bundle tree-shaken — see the
   ~10 MB binary budget. Adding a glyph to the UI means adding it here first.
   If the product ever gets its own icon set, replace this map and nothing else
   in the system changes. */
import {
  Anchor,
  ArrowDownToLine,
  Bell,
  Bot,
  Bug,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsRight,
  CircleDashed,
  Clock,
  Columns3,
  Copy,
  CornerDownRight,
  Dot,
  File,
  FileCode,
  FilePen,
  FileX,
  Folder,
  FolderGit2,
  FolderOpen,
  Gauge,
  GitBranch,
  GitFork,
  GitMerge,
  Inbox,
  Info,
  Layers,
  LoaderCircle,
  Lock,
  MessageCircleQuestion,
  Milestone,
  Minus,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  Paperclip,
  Pause,
  Pin,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Settings2,
  Sparkles,
  Square,
  SquareCheck,
  Tag,
  Terminal,
  Trash2,
  TriangleAlert,
  User,
  Wrench,
  X
} from 'lucide'

export const iconNodes = {
  // agents and people
  bot: Bot,
  user: User,
  'circle-dashed': CircleDashed,
  terminal: Terminal,
  'trash-2': Trash2,

  // the dependency graph
  lock: Lock,
  'git-fork': GitFork,
  'corner-down-right': CornerDownRight,

  // scope
  'git-branch': GitBranch,
  'git-merge': GitMerge,
  'folder-git-2': FolderGit2,
  'file-pen': FilePen,

  // attachments
  paperclip: Paperclip,

  // files
  file: File,
  'file-code': FileCode,
  folder: Folder,
  'folder-open': FolderOpen,

  // status
  'triangle-alert': TriangleAlert,
  // The run configuration that exists and cannot be parsed. A triangle would
  // have been the obvious choice and is the wrong one: the project row already
  // draws a muted triangle for a folder with no bd tracker, and a red one on
  // its own beside it would leave two states of a project told apart by colour
  // and nothing else. A page silhouette says which of the two is about a file.
  'file-x': FileX,
  'loader-circle': LoaderCircle,
  check: Check,
  x: X,
  info: Info,
  dot: Dot,
  // bd's own statuses that are not reserved here: deferred, pinned, hooked.
  clock: Clock,
  pin: Pin,
  anchor: Anchor,
  // The design system asks for "message-circle-question-mark"; lucide 0.469 still
  // calls it MessageCircleQuestion (renamed upstream later). Keep the DS name.
  'message-circle-question-mark': MessageCircleQuestion,

  // log and tools
  pause: Pause,
  play: Play,
  // Stop, in the run bar: a filled square is what every transport control
  // in the world uses, and there is no `stop` glyph in lucide.
  square: Square,
  'arrow-down-to-line': ArrowDownToLine,
  wrench: Wrench,
  search: Search,
  'refresh-cw': RefreshCw,
  copy: Copy,

  // shell
  'panel-left-close': PanelLeftClose,
  'panel-left-open': PanelLeftOpen,
  'panel-right-close': PanelRightClose,
  'panel-right-open': PanelRightOpen,
  'chevron-down': ChevronDown,
  'chevron-right': ChevronRight,
  'chevrons-right': ChevronsRight,
  bell: Bell,
  settings: Settings,
  'settings-2': Settings2,
  plus: Plus,
  minus: Minus,
  gauge: Gauge,
  inbox: Inbox,
  'columns-3': Columns3,

  // issue types
  bug: Bug,
  sparkles: Sparkles,
  layers: Layers,
  'square-check': SquareCheck,
  milestone: Milestone,
  tag: Tag
}
