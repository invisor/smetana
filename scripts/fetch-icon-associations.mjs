#!/usr/bin/env node
/* Vendors the Catppuccin VS Code icon theme into two artifacts under src/:

     catppuccinAssociations.json — which file name, which extension and which
       folder name gets which icon
     catppuccinBodies.json — the icons themselves, plus the two palettes

   Both come from https://github.com/catppuccin/vscode-icons (MIT), taken as one
   repository tarball rather than 656 requests.

   The icons are the `icons/css-variables/` build, whose SVGs are written against
   `var(--vscode-ctp-<colour>)` rather than against hexes — one semantic colour
   name per icon, fifteen names in all. That is what makes two themes affordable:
   the bodies are stored once with the placeholders left in, and the palette is
   fifteen hexes per flavour, so the app substitutes at render time instead of
   carrying a second compiled copy of every icon. The repository ships those
   compiled copies (`icons/latte/`, `icons/macchiato/`, …) and taking them would
   have doubled the payload to draw exactly the same pixels.

   Upstream's association tables are TypeScript keyed by *icon* name and a lookup
   wants the inverse, so this script inverts them. The alternative was copying
   somebody's port of the same two files, which is a copy of a copy with no way
   to refresh it.

   Pinned to a ref rather than tracking a branch, for the reason fetch-bd.mjs
   pins a version: tables that change under a checked-in artifact make the two
   disagree with no diff to show for it. Refreshing means moving REF and running
   this again — the output is committed. */
import { execFileSync } from 'node:child_process'
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

/* catppuccin/vscode-icons, main as of 2026-08-17. */
const REF = 'main'
const TARBALL = `https://codeload.github.com/catppuccin/vscode-icons/tar.gz/refs/heads/${REF}`
/* The palettes are read out of the repository's own compiled builds rather than
   from `@catppuccin/palette`, which publishes them as JavaScript and would be a
   second download to keep in step. Every icon exists in `icons/css-variables/`
   and again in `icons/<flavour>/`, byte-for-byte identical but for the colours,
   so pairing the two positionally yields the flavour's hex for each colour name
   — and yields it from the very build these bodies come from. */

/* Latte for the light theme, Macchiato for the dark one. Frappé and Mocha are
   the two this app has no use for: the choice is which of the four reads on our
   own two surfaces, not which flavour somebody prefers. */
const FLAVOURS = { light: 'latte', dark: 'macchiato' }

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const OUT_ASSOCIATIONS = join(root, 'src', 'catppuccinAssociations.json')
const OUT_BODIES = join(root, 'src', 'catppuccinBodies.json')

/* The upstream files are TypeScript, not JSON: an object literal with a type
   annotation above it and single-quoted keys. Rather than pull in a TS parser
   for two files whose shape is this narrow, read the entries with a regex over
   the one construct they use — a key, then a body of `listName: [ ... ]`. A
   malformed match is loud rather than silent: the counts are asserted below. */
function parseEntries(source) {
  const entries = {}
  const block = /^ {2}'?([\w.@+-]+)'?: \{\n([\s\S]*?)^ {2}\},$/gm
  for (const [, icon, body] of source.matchAll(block)) {
    const lists = {}
    for (const [, list, items] of body.matchAll(/^ {4}(\w+): \[([\s\S]*?)\],$/gm)) {
      lists[list] = [...items.matchAll(/'([^']*)'/g)].map((m) => m[1])
    }
    entries[icon] = lists
  }
  return entries
}

/* Later wins is wrong here and first wins is right: the tables are sorted by
   icon name, so a duplicate extension claimed by two icons would otherwise be
   decided by the alphabet at the far end of the file rather than at the near
   one. Neither is meaningful, but only one of them is stable. */
function invert(entries, listName) {
  const out = {}
  for (const [icon, lists] of Object.entries(entries)) {
    for (const key of lists[listName] ?? []) {
      const lower = key.toLowerCase()
      if (!(lower in out)) out[lower] = icon
    }
  }
  return out
}

/* What travels is the inside of the `<svg>` and nothing else: the wrapper is
   written by the app, which is what stops 656 copies of the same attributes from
   riding along. The tabs and newlines upstream indents with are worth 60 KB
   across the set and nothing at all on screen. */
function bodyOf(svg) {
  const inner = svg.replace(/^[\s\S]*?<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '')
  return (
    inner
      .replace(/\s*\n\s*/g, '')
      /* `var(--vscode-ctp-blue)` is 21 characters and appears 1300 times across
         the set — 27 KB of one repeated string. `$blue$` says the same thing and
         is shorter than the hex it will be replaced by, so the placeholder form
         of this artifact is smaller than a compiled one would be, never mind two
         of them. */
      .replace(/var\(--vscode-ctp-(\w+)\)/g, '$$$1$$')
      .trim()
  )
}

const work = await mkdtemp(join(tmpdir(), 'catppuccin-icons-'))
try {
  const archive = join(work, 'icons.tar.gz')
  execFileSync('curl', ['-sL', TARBALL, '-o', archive])
  execFileSync('tar', ['xzf', archive, '-C', work])

  const [checkout] = (await readdir(work)).filter((name) => name.startsWith('vscode-icons-'))
  if (!checkout) throw new Error('the tarball did not unpack into a checkout')
  const repo = join(work, checkout)

  const [fileSource, folderSource] = await Promise.all([
    readFile(join(repo, 'src/defaults/fileIcons.ts'), 'utf8'),
    readFile(join(repo, 'src/defaults/folderIcons.ts'), 'utf8')
  ])

  const fileEntries = parseEntries(fileSource)
  const folderEntries = parseEntries(folderSource)

  /* A parse that quietly matched nothing would write an empty table and leave
     the tree drawing the default page for every file — which looks like a design
     decision rather than a broken script. */
  if (Object.keys(fileEntries).length < 200) throw new Error('file icon table looks empty')
  if (Object.keys(folderEntries).length < 50) throw new Error('folder icon table looks empty')

  const associations = {
    source: `catppuccin/vscode-icons@${REF}`,
    license: 'MIT',
    fileNames: invert(fileEntries, 'fileNames'),
    fileExtensions: invert(fileEntries, 'fileExtensions'),
    languageIds: invert(fileEntries, 'languageIds'),
    folderNames: invert(folderEntries, 'folderNames')
  }

  const iconDir = join(repo, 'icons/css-variables')
  const svgs = (await readdir(iconDir)).filter((name) => name.endsWith('.svg'))
  if (svgs.length < 500) throw new Error(`only ${svgs.length} icons in the css-variables build`)

  const icons = {}
  const used = new Set()
  const namesByIcon = {}
  for (const file of svgs) {
    const svg = await readFile(join(iconDir, file), 'utf8')
    icons[file.slice(0, -4)] = bodyOf(svg)
    const names = [...svg.matchAll(/var\(--vscode-ctp-(\w+)\)/g)].map((m) => m[1])
    namesByIcon[file] = names
    for (const name of names) used.add(name)
  }

  const colours = {}
  for (const [theme, flavour] of Object.entries(FLAVOURS)) {
    const found = {}
    for (const [file, names] of Object.entries(namesByIcon)) {
      if (names.length === 0) continue
      const compiled = await readFile(join(repo, 'icons', flavour, file), 'utf8')
      const hexes = [...compiled.matchAll(/#[0-9a-f]{6}/gi)].map((m) => m[0].toLowerCase())
      /* A file whose two builds disagree on how many colours they carry is not
         a pairing this can trust, so it is skipped rather than guessed at; every
         colour appears in dozens of icons and is picked up from the rest. */
      if (hexes.length !== names.length) continue
      names.forEach((name, i) => {
        const hex = hexes[i]
        if (found[name] && found[name] !== hex) {
          throw new Error(`${flavour}: ${name} read as both ${found[name]} and ${hex} (${file})`)
        }
        found[name] = hex
      })
    }
    const missing = [...used].filter((name) => !found[name])
    /* A colour left unread would be substituted as `undefined` into an SVG
       attribute, where it draws nothing and says nothing. */
    if (missing.length) throw new Error(`${flavour} is missing ${missing.join(', ')}`)
    colours[theme] = Object.fromEntries([...used].sort().map((name) => [name, found[name]]))
  }

  await writeFile(OUT_ASSOCIATIONS, `${JSON.stringify(associations)}\n`)
  await writeFile(
    OUT_BODIES,
    `${JSON.stringify({ source: associations.source, license: 'MIT', colours, icons })}\n`
  )

  const count = (table) => Object.keys(associations[table]).length
  console.log(
    `wrote ${OUT_ASSOCIATIONS}: ${count('fileNames')} names, ${count('fileExtensions')} extensions, ` +
      `${count('languageIds')} language ids, ${count('folderNames')} folder names`
  )
  console.log(
    `wrote ${OUT_BODIES}: ${Object.keys(icons).length} icons, ` +
      `${used.size} colours per flavour (${Object.values(FLAVOURS).join(', ')})`
  )
} finally {
  await rm(work, { recursive: true, force: true })
}
