/* What a file or a folder is drawn as, by its name alone.

   Not under `components/files/`, and for the reason `paths.js` beside it is not
   under anything either: three different parts of the interface want this at
   once — the tree's rows (`files/FileTreeRow.vue`), the document tabs
   (`stores/tabs.js`) and the Git panel's change list (`git/ChangeList.vue`) —
   so there is no one part of the interface to file it under. Reaching into a
   component's directory from a store would also have pointed the dependency the
   wrong way round.

   No group carries a colour, and none of the callers gives it one: the tree and
   the change list draw every glyph in `--text-muted`, and a document tab hands
   it nothing at all, so it inherits the tab's own foreground the way its label
   does — a tab is two things on a strip rather than a mark in a list, and a
   muted glyph beside an active tab's bright label would read as disabled. The
   saturated range belongs to status (`status/status.js`), and in the two lists
   the row's one colour is already spoken for — the tree's git letter and the
   change list's kind. What tells the groups apart is silhouette, the same way a
   type badge is told from a status badge.

   The groups are by meaning and not by language, which is a limit of the icon
   set as much as a decision: lucide ships no language logos, and a glyph
   invented per language reads as noise at 13px. Every name below resolves to one
   of six, and a name none of them claims is an ordinary outcome that draws the
   plain page — the stance `editor/languages.js` takes for an extension it cannot
   highlight. */

const CODE = 'file-code'
const TEXT = 'file-text'
const DATA = 'braces'
const MANIFEST = 'package'
const IMAGE = 'image'
const ARCHIVE = 'file-archive'
const PLAIN = 'file'

/* Both maps below are prototype-free, and that is a correctness fix rather than
   a habit: a plain object literal answers `constructor` with the `Object`
   function, and `fileIcon('constructor')` would hand `Icon` a function where a
   glyph name belongs. A file called `constructor` is unremarkable in a
   JavaScript tree. `Object.hasOwn` would say the same thing and is ES2022,
   above this build's `es2021` target. */
const map = (entries) => Object.assign(Object.create(null), entries)

/* Anything a person edits as source, markup included: a stylesheet and a
   template are written the way code is, and a row that split them off would be
   drawing the difference between two files somebody opens for the same
   reason. */
const BY_EXTENSION = map({
  js: CODE,
  mjs: CODE,
  cjs: CODE,
  jsx: CODE,
  ts: CODE,
  tsx: CODE,
  vue: CODE,
  rs: CODE,
  py: CODE,
  go: CODE,
  rb: CODE,
  php: CODE,
  java: CODE,
  kt: CODE,
  swift: CODE,
  c: CODE,
  h: CODE,
  cc: CODE,
  cpp: CODE,
  hpp: CODE,
  sh: CODE,
  bash: CODE,
  zsh: CODE,
  sql: CODE,
  html: CODE,
  htm: CODE,
  css: CODE,
  scss: CODE,
  xml: CODE,

  md: TEXT,
  markdown: TEXT,
  txt: TEXT,
  rst: TEXT,
  adoc: TEXT,

  json: DATA,
  jsonc: DATA,
  toml: DATA,
  yaml: DATA,
  yml: DATA,
  ini: DATA,
  cfg: DATA,
  conf: DATA,
  env: DATA,

  /* Every lock file there is, whatever it locks: `Cargo.lock`, `yarn.lock`,
     `bun.lock`. It is the manifest's other half and reads as the same thing. */
  lock: MANIFEST,

  png: IMAGE,
  jpg: IMAGE,
  jpeg: IMAGE,
  gif: IMAGE,
  svg: IMAGE,
  webp: IMAGE,
  avif: IMAGE,
  ico: IMAGE,

  zip: ARCHIVE,
  tar: ARCHIVE,
  gz: ARCHIVE,
  tgz: ARCHIVE,
  bz2: ARCHIVE,
  xz: ARCHIVE,
  '7z': ARCHIVE
})

/* Files whose name matters more than their extension — the same split
   `editor/languages.js` makes, and for the same reason: `Cargo.toml` is what the
   project is made of where the toml beside it is somebody's own settings. Half
   of these have no extension at all. */
const BY_NAME = map({
  'package.json': MANIFEST,
  'package-lock.json': MANIFEST,
  'pnpm-lock.yaml': MANIFEST,
  'cargo.toml': MANIFEST,
  'pyproject.toml': MANIFEST,
  'go.mod': MANIFEST,
  'go.sum': MANIFEST,
  'gemfile': MANIFEST,

  dockerfile: CODE,
  makefile: CODE,
  justfile: CODE,

  license: TEXT,
  licence: TEXT,
  readme: TEXT,
  changelog: TEXT,
  authors: TEXT,
  notice: TEXT
})

/* Both separators, for the reason `paths.js` gives: two of the three callers
   hold a path rather than a name, and one of the target webviews is WebView2. */
const nameOf = (path) => String(path ?? '').split(/[/\\]/).filter(Boolean).pop() ?? ''

export function fileIcon(path) {
  const name = nameOf(path).toLowerCase()
  if (!name) return PLAIN

  const byName = BY_NAME[name]
  if (byName) return byName

  /* `dot > 0` and not `>= 0`: a leading dot names a file rather than an
     extension, which is what leaves `.gitignore` to the rule below. */
  const dot = name.lastIndexOf('.')
  const byExtension = dot > 0 ? BY_EXTENSION[name.slice(dot + 1)] : undefined
  if (byExtension) return byExtension

  /* A dotfile nothing else claimed is configuration. One rule instead of a list
     that would have to grow for every `.npmrc`, `.prettierrc` and `.zshrc`
     anybody keeps in a project — and it is what catches `.env.local`, whose
     extension nothing claims. It is deliberately last, so a dotfile with a known
     extension (`.eslintrc.json`, `.github/…`) is drawn by that extension. */
  if (name.startsWith('.')) return DATA

  return PLAIN
}

/* A folder is a folder, and no name is special — including the one that looks
   like it should be. `.git` was written here first and taken back out: the tree
   never draws it (`files::model::skip_in_tree` is the only thing that directory
   is), and the change list never sees it either, so the branch was a rule
   nothing could reach. The generated directories somebody would reach for next
   — `node_modules`, `target`, `dist` — are a question about how a row is dimmed
   rather than which glyph it takes, and there is no such dimming in the app yet.

   It stays a function beside `fileIcon` all the same: both callers ask the same
   question of a name, and a folder that ever does earn a glyph has one place to
   earn it in. The trailing separator is why the change list can call it at all —
   `--untracked-files=normal` reports an untracked directory as one record ending
   in `/`. */
export const folderIcon = (path, expanded = false) => (expanded ? 'folder-open' : 'folder')
