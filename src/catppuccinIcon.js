/* EXPERIMENT — variant A of the file-icon estimate, on this branch only.

   What a file or a folder is drawn as, using the Catppuccin VS Code icon set
   (MIT) exactly as it ships: 659 multicolour icons, resolved by name, and handed
   to an `<img>` as a `data:` URL. It is the same shape Terax uses.

   Three of this repository's written rules are suspended here, deliberately and
   visibly, which is the whole point of trying it:

   1. Colour. Not one body in the set uses `currentColor` — `info.json` says
      `"palette": true` — so 19 fixed hexes from the Catppuccin Macchiato palette
      enter the interface. `CLAUDE.md` says the saturated range belongs to status
      and that a missing token is a design-system question.
   2. The one raster. An `<img>` is not a raster, but the rule it breaks is the
      same one's second half: a `data:` URL is opaque to the stylesheet, so
      `var(--token)` does not resolve inside it and nothing repaints when
      `data-theme` flips. The set is dark-flavoured, so in the light theme the
      dominant colour sits at 1.48:1 against white.
   3. `core/icons.js` is no longer the only file that names an icon set, and this
      one is not tree-shaken: the whole `icons.json` is imported, as measured.

   `src/fileIcon.js` beside this file is the monochrome rule this replaces. It is
   deliberately left in place and unused, so the two can be compared and so that
   dropping this experiment is one revert. */
import catppuccinIcons from '@iconify-json/catppuccin/icons.json'
import tables from './catppuccinAssociations.json'

/* `JSON.parse` builds ordinary objects, so every table below answers
   `constructor` with the `Object` function and `toString` with a function too —
   and a file called `constructor` is unremarkable in a JavaScript tree. The
   tables are re-hung on a null prototype once, here, rather than guarded at
   each of the four lookups. */
const associations = {
  fileNames: Object.assign(Object.create(null), tables.fileNames),
  fileExtensions: Object.assign(Object.create(null), tables.fileExtensions),
  languageIds: Object.assign(Object.create(null), tables.languageIds),
  folderNames: Object.assign(Object.create(null), tables.folderNames)
}

const DEFAULT_FILE = 'file'
const DEFAULT_FOLDER = 'folder'
const DEFAULT_FOLDER_OPEN = 'folder-open'

/* The set draws on a 16×16 grid, where lucide's is 24×24 — the number is in
   `info.json` rather than in `icons.json`, which carries no root width. */
const VIEWBOX = 16

/* Building a `data:` URL means percent-encoding an SVG per row per render, so
   the answers are kept. The set is 659 icons and a tree draws a few dozen, so
   the cache is bounded by what is actually on screen. */
const urlCache = new Map()

/* Catppuccin's own manifest names icons `folder_src`, while the iconify export
   normalises every name to a hyphenated slug. The association tables come from
   the manifest, so the two have to be brought together. */
const slug = (name) => name.replace(/_/g, '-')

function body(name) {
  const key = slug(name)
  const direct = catppuccinIcons.icons[key]
  if (direct) return direct.body
  const alias = catppuccinIcons.aliases?.[key]
  const parent = alias ? catppuccinIcons.icons[alias.parent] : undefined
  return parent ? parent.body : null
}

function urlFor(name) {
  if (urlCache.has(name)) return urlCache.get(name)
  const svg = body(name)
  const url = svg
    ? `data:image/svg+xml;utf8,${encodeURIComponent(
        `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${VIEWBOX} ${VIEWBOX}">${svg}</svg>`
      )}`
    : ''
  urlCache.set(name, url)
  return url
}

/* Both separators, the reason `paths.js` gives: two of the three callers hold a
   path rather than a name. */
const nameOf = (path) => String(path ?? '').split(/[/\\]/).filter(Boolean).pop() ?? ''

export function fileIconUrl(path) {
  const name = nameOf(path).toLowerCase()

  const byName = associations.fileNames[name]
  if (byName) {
    const url = urlFor(byName)
    if (url) return url
  }

  /* Compound extensions, shortest suffix last: `component.spec.ts` is asked as
     `spec.ts` before `ts`, which is what lets the set draw a test file as a test
     rather than as TypeScript. The first dot and not the last, then, and the
     walk is what makes the difference. */
  let ext = name.includes('.') ? name.slice(name.indexOf('.') + 1) : ''
  while (ext) {
    const byExtension = associations.fileExtensions[ext]
    if (byExtension) {
      const url = urlFor(byExtension)
      if (url) return url
    }
    /* The language-id table is upstream's third route, and it is reachable here
       only where a language's id happens to be spelled like the extension
       (`json`, `xml`, `sql`). VS Code's own extension → language-id map is what
       would open the rest of it, and porting that is variant B's problem. */
    const byLanguage = associations.languageIds[ext]
    if (byLanguage) {
      const url = urlFor(byLanguage)
      if (url) return url
    }
    const dot = ext.indexOf('.')
    if (dot === -1) break
    ext = ext.slice(dot + 1)
  }

  return urlFor(DEFAULT_FILE)
}

export function folderIconUrl(path, expanded = false) {
  const name = nameOf(path).toLowerCase()

  const mapped = associations.folderNames[name]
  if (mapped) {
    /* The `folder-` prefix is the load-bearing part. Upstream's table is keyed
       by icon *basename* with the prefix stripped ("src", "animation"), while
       the set names them `folder-src` and `folder-animation`, each with an
       `-open` twin — all 113 of them resolve this way and only 50 resolve
       without the prefix. Taking the value as it stands, the way the resolver
       this was modelled on does, loses 63 folders to the default and draws the
       other 50 as the *file* icon of the same name: a `gradle` folder comes out
       as a gradle file. */
    const url = urlFor(expanded ? `folder-${slug(mapped)}-open` : `folder-${slug(mapped)}`)
    if (url) return url
  }

  return urlFor(expanded ? DEFAULT_FOLDER_OPEN : DEFAULT_FOLDER)
}
