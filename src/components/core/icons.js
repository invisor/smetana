/* The glyph vocabulary, and the only place Lucide is named.
   Registering icons explicitly is what keeps the bundle tree-shaken — see the
   ~10 MB binary budget. Adding a glyph to the UI means adding it here first.

   It is no longer the only icon source in the tree, and the split is by
   question rather than by taste: this file answers "what does this control
   mean", in one colour, from a list a person can read; `src/catppuccinIcon.js`
   answers "what kind of file is this", from a set of 656 named after
   languages and tools. A vocabulary of that size cannot be a hand-kept list,
   and a control's glyph cannot be a colour somebody else chose. */
import {
  Anchor,
  ArrowDown,
  ArrowDownToLine,
  ArrowRightToLine,
  ArrowUp,
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
  Ellipsis,
  ExternalLink,
  File,
  FileCode,
  FilePen,
  FilePlus,
  FileX,
  Folder,
  FolderGit2,
  FolderOpen,
  FolderPlus,
  Gauge,
  GitBranch,
  GitBranchPlus,
  GitCompare,
  GitFork,
  GitGraph,
  GitMerge,
  HardDrive,
  Inbox,
  Info,
  Layers,
  LoaderCircle,
  Lock,
  MessageCircleQuestion,
  MessageSquare,
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
  Star,
  SquareCheck,
  SquarePen,
  Tag,
  Terminal,
  Trash2,
  TriangleAlert,
  User,
  UserCheck,
  Wrench,
  X
} from 'lucide'

export const iconNodes = {
  // agents and people
  bot: Bot,
  user: User,
  /* A person with a tick: work that is finished and merged and still owes
     somebody a look. `status/status.js` draws it for the `human_check`
     column. */
  'user-check': UserCheck,
  'circle-dashed': CircleDashed,
  terminal: Terminal,
  'trash-2': Trash2,

  // the dependency graph
  lock: Lock,
  'git-fork': GitFork,
  'corner-down-right': CornerDownRight,

  // scope
  'git-branch': GitBranch,
  'git-branch-plus': GitBranchPlus,
  'git-merge': GitMerge,
  /* Rebase, beside merge in the branch list. Lucide ships no rebase glyph, and
     of what it does ship this is the one about the *shape* of the history —
     which is the whole difference between the two operations. `git-compare` is
     taken by the diff tab and `git-pull-request-arrow` would name something
     this app has no idea about. */
  'git-graph': GitGraph,
  // a diff tab: two columns of one file, HEAD against the working tree
  'git-compare': GitCompare,
  'folder-git-2': FolderGit2,
  'file-pen': FilePen,
  /* A branch somebody pinned to the top of the Git panel's list. It is drawn in
     the leading icon's place, instead of `git-branch` and never beside it — a
     sixth glyph in front of a name would put the marked rows' names out of line
     with every other row's. The same glyph names the menu item that puts it
     there and the one that takes it away: the item's own label is what says
     which of the two a press does. */
  star: Star,

  // attachments
  paperclip: Paperclip,

  /* files. Deliberately few: what a *named* file or folder is drawn as is not
     lucide's job any more but `src/catppuccinIcon.js`'s, which resolves a name
     against the Catppuccin set. These are the page and the folder as ordinary
     interface glyphs — a menu item, an empty state — and the tree does not
     reach for them. */
  file: File,
  'file-code': FileCode,
  folder: Folder,
  'folder-open': FolderOpen,
  /* The file tree's menu, where the pair is the two things that can be made.
     The plain page and folder with a plus and nothing else: what is about to be
     made has no name yet, so there is no file type to draw — and the moment it
     has one, the row that appears in the tree is drawn by the other icon source
     entirely. */
  'file-plus': FilePlus,
  'folder-plus': FolderPlus,

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
  // The About tab's link. The glyph is the whole of what says the link leaves
  // the app for the person's own browser, since there is no address bar here to
  // make that obvious afterwards.
  'external-link': ExternalLink,
  dot: Dot,
  // bd's own statuses that are not reserved here: deferred, pinned, hooked.
  clock: Clock,
  pin: Pin,
  anchor: Anchor,
  // The design system asks for "message-circle-question-mark"; lucide 0.469 still
  // calls it MessageCircleQuestion (renamed upstream later). Keep the DS name.
  'message-circle-question-mark': MessageCircleQuestion,
  /* The caption over an opened session card's first prompt. A plain speech
     bubble and deliberately not the question mark above it: that glyph is the
     card menu's "Answer questions" on a parked task, where it means somebody is
     being asked something, and this one only says that what follows is a
     message rather than a field. */
  'message-square': MessageSquare,

  // log and tools
  pause: Pause,
  play: Play,
  // Stop, in the run bar: a filled square is what every transport control
  // in the world uses, and there is no `stop` glyph in lucide.
  square: Square,
  'arrow-down-to-line': ArrowDownToLine,
  // The Git panel's two remote verbs, and the marks on a branch row that is
  // behind or ahead of its upstream. The bare arrows and not the `-to-line`
  // pair beside them: those two mean "all the way", and this is a direction.
  'arrow-down': ArrowDown,
  'arrow-up': ArrowUp,
  // A whole column moved into the queue, in the deferred header. Deliberately
  // not the play: that glyph means a run starts, and this one only changes a
  // status. The line at the end is what makes it "all the way into the queue"
  // rather than a nudge in some direction.
  'arrow-right-to-line': ArrowRightToLine,
  wrench: Wrench,
  // The card menu's Edit. `file-pen` is a page with a pen on it and belongs to
  // the file it is about; this one is the bare verb, which is what a row acting
  // on an issue wants.
  'square-pen': SquarePen,
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
  // The card's overflow menu, where the play used to be. Three dots and not a
  // vertical `ellipsis-vertical`: the card's top row is horizontal and the
  // glyph it replaced sat in it the same way.
  ellipsis: Ellipsis,
  'chevrons-right': ChevronsRight,
  bell: Bell,
  /* The storage source's glyph: the attachment store's own weight. */
  'hard-drive': HardDrive,
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
