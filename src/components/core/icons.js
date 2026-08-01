/* The glyph vocabulary, and the only place Lucide is named.
   Registering icons explicitly is what keeps the bundle tree-shaken — see the
   ~10 MB binary budget. Adding a glyph to the UI means adding it here first.
   If the product ever gets its own icon set, replace this map and nothing else
   in the system changes. */
import {
  ArrowDownToLine,
  Bell,
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsRight,
  CircleDashed,
  Columns3,
  Copy,
  CornerDownRight,
  Dot,
  File,
  FileCode,
  FilePen,
  Folder,
  FolderGit2,
  FolderOpen,
  Gauge,
  GitBranch,
  GitFork,
  Inbox,
  Info,
  LoaderCircle,
  Lock,
  MessageCircleQuestion,
  Minus,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Terminal,
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

  // the dependency graph
  lock: Lock,
  'git-fork': GitFork,
  'corner-down-right': CornerDownRight,

  // scope
  'git-branch': GitBranch,
  'folder-git-2': FolderGit2,
  'file-pen': FilePen,

  // files
  file: File,
  'file-code': FileCode,
  folder: Folder,
  'folder-open': FolderOpen,

  // status
  'triangle-alert': TriangleAlert,
  'loader-circle': LoaderCircle,
  check: Check,
  x: X,
  info: Info,
  dot: Dot,
  // The design system asks for "message-circle-question-mark"; lucide 0.469 still
  // calls it MessageCircleQuestion (renamed upstream later). Keep the DS name.
  'message-circle-question-mark': MessageCircleQuestion,

  // log and tools
  pause: Pause,
  play: Play,
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
  plus: Plus,
  minus: Minus,
  gauge: Gauge,
  inbox: Inbox,
  'columns-3': Columns3
}
