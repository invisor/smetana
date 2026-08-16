/* What a file or a folder is drawn as, using the Catppuccin VS Code icon set
   (MIT): 656 multicolour icons resolved by name and handed to an `<img>` as a
   `data:` URL. The tree's rows, the document tabs and the Git panel's change
   list all draw from here, which is why it sits at the top of `src/` rather
   than under any one of them — the reason `paths.js` gives about itself.

   The set ships as SVGs written against `var(--vscode-ctp-<colour>)` — one
   semantic colour name per icon, sixteen names in all — and
   `scripts/fetch-icon-associations.mjs` vendors them with the placeholders left
   in, beside both flavours' hexes. So the colours are substituted here, per
   theme, and the two palettes cost one copy of the artwork rather than two:
   Latte for the light theme, Macchiato for the dark one.

   Two of this repository's written rules are bought out here rather than kept,
   and both were weighed with measurements in hand:

   1. Colour. Sixteen hues from a foreign palette enter the interface, where
      `CLAUDE.md` otherwise says the saturated range belongs to status. The cost
      is real and it is where status is most load-bearing: in the change list a
      modified `.js` puts the status letter and the icon **within one degree of
      hue** of each other, and on a tab the peach of a Rust icon is 17 degrees
      from the amber dot that means "unsaved". Nine of the sixteen Latte hues
      also sit under 3:1 on `--surface` — legible, and visibly softer than the
      label beside them.
   2. `core/icons.js` is no longer the only file that names an icon set, and
      this one is not tree-shaken: the whole vendored body table is imported,
      which is what the bundle grew by. A hand-kept list of 656 names resolved
      at runtime is not a thing that can be tree-shaken at all.

   What is *not* given up is the theme. The icons repaint on `data-theme`,
   because the palette is applied here rather than baked in — which is the whole
   reason the first version of this file was thrown away. It imported the
   compiled Macchiato build, and Macchiato is a dark palette: its dominant
   colour measured 1.38:1 against `--surface`, so in the light theme most of the
   tree's icons were simply not on screen. The same measurement now reads
   7.41:1. */
import bodies from './catppuccinBodies.json'
import tables from './catppuccinAssociations.json'

/* Upstream names the three fallbacks with a leading underscore, and its file
   icons with hyphens where its folder icons take an underscore after the
   `folder` prefix. The association tables are keyed the same way, so nothing
   here rewrites a name — it only adds the prefix a folder's table entry has had
   stripped. */
const DEFAULT_FILE = '_file'
const DEFAULT_FOLDER = '_folder'
const DEFAULT_FOLDER_OPEN = '_folder_open'

/* The set draws on a 16 grid. */
const VIEWBOX = 16

/* What the vendoring script leaves in place of a colour: `$blue$` for what
   upstream writes as `var(--vscode-ctp-blue)`. Shorter than the hex that
   replaces it, which is what keeps one artifact with two palettes smaller than
   a single compiled flavour would be. */
const PLACEHOLDER = /\$(\w+)\$/g

/* `JSON.parse` builds ordinary objects, so every table would answer
   `constructor` with the `Object` function and `toString` with a function too —
   and a file called `constructor` is unremarkable in a JavaScript tree. The
   tables are re-hung on a null prototype once, here, rather than guarded at each
   of the five lookups. */
const map = (table) => Object.assign(Object.create(null), table)

const associations = {
  fileNames: map(tables.fileNames),
  fileExtensions: map(tables.fileExtensions),
  languageIds: map(tables.languageIds),
  folderNames: map(tables.folderNames)
}
const icons = map(bodies.icons)
const palettes = map(bodies.colours)

/* Percent-encoding an SVG per row per render would be wasteful, and per theme is
   the key: the same icon is two different documents in the two themes. The cache
   is bounded by what has actually been on screen. */
const urlCache = new Map()

function urlFor(name, theme) {
  const key = `${theme}:${name}`
  const cached = urlCache.get(key)
  if (cached !== undefined) return cached

  const body = icons[name]
  const colours = palettes[theme] ?? palettes.dark
  const url = body
    ? `data:image/svg+xml;utf8,${encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${VIEWBOX} ${VIEWBOX}">${body.replace(
          PLACEHOLDER,
          (whole, colour) => colours[colour] ?? whole
        )}</svg>`
      )}`
    : ''
  urlCache.set(key, url)
  return url
}

/* Both separators, the reason `paths.js` gives: two of the three callers hold a
   path rather than a name. */
const nameOf = (path) => String(path ?? '').split(/[/\\]/).filter(Boolean).pop() ?? ''

/* The theme is an argument rather than something read in here, so the rule stays
   pure and a test can ask for either palette without touching a document. The
   callers pass `documentTheme.value`, and reading that ref inside their own
   computed is what makes an icon repaint when the theme flips. */
export function fileIconUrl(path, theme = 'dark') {
  const name = nameOf(path).toLowerCase()

  const byName = associations.fileNames[name]
  if (byName) {
    const url = urlFor(byName, theme)
    if (url) return url
  }

  /* Compound extensions, longest suffix first: `component.d.ts` is asked as
     `d.ts` before `ts`, which is what lets the set draw a declaration file as
     one. Hence the walk from the first dot rather than a single look at the
     last. */
  let ext = name.includes('.') ? name.slice(name.indexOf('.') + 1) : ''
  while (ext) {
    const byExtension = associations.fileExtensions[ext]
    if (byExtension) {
      const url = urlFor(byExtension, theme)
      if (url) return url
    }
    /* The language-id table is upstream's third route, reachable here only where
       a language's id happens to be spelled like the extension (`json`, `xml`,
       `sql`). VS Code's own extension → language-id map is what would open the
       rest of it, and porting that is variant B's problem. */
    const byLanguage = associations.languageIds[ext]
    if (byLanguage) {
      const url = urlFor(byLanguage, theme)
      if (url) return url
    }
    const dot = ext.indexOf('.')
    if (dot === -1) break
    ext = ext.slice(dot + 1)
  }

  return urlFor(DEFAULT_FILE, theme)
}

export function folderIconUrl(path, expanded = false, theme = 'dark') {
  const name = nameOf(path).toLowerCase()

  const mapped = associations.folderNames[name]
  if (mapped) {
    /* The `folder_` prefix is the load-bearing part. Upstream's table is keyed
       by icon basename with the prefix stripped ("src", "animation"), while the
       files are `folder_src` and `folder_src_open`. Taking the value as it
       stands, the way the resolver this was modelled on does, loses every folder
       whose name is not also a file icon's — and draws the rest as that file:
       a `gradle` folder comes out as a gradle file. */
    const url = urlFor(expanded ? `folder_${mapped}_open` : `folder_${mapped}`, theme)
    if (url) return url
  }

  return urlFor(expanded ? DEFAULT_FOLDER_OPEN : DEFAULT_FOLDER, theme)
}
