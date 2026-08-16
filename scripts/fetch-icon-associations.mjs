#!/usr/bin/env node
/* Vendors the Catppuccin VS Code icon theme's association tables into
   src/catppuccinAssociations.json — which file name, which extension and which
   folder name gets which icon.

   This is the experiment's data half. The icon bodies themselves come from the
   `@iconify-json/catppuccin` package at build time; only the associations live
   upstream in TypeScript and have no published machine-readable form, hence
   this script. The alternative was copying somebody's port of the same two
   files, which would have been a copy of a copy with no way to refresh it.

   Upstream is https://github.com/catppuccin/vscode-icons (MIT), files
   `src/defaults/fileIcons.ts` and `src/defaults/folderIcons.ts`. They are typed
   objects keyed by *icon* name, listing the extensions and file names that
   resolve to it; a lookup wants the inverse, so this script inverts them and
   writes one JSON artifact.

   It is pinned to a commit rather than to `main`, for the reason fetch-bd.mjs
   pins a version: a table that changes under a checked-in artifact makes the
   two disagree with no diff to show for it. Refreshing means moving the SHA
   here and running the script again — the output is committed. */
import { writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

/* catppuccin/vscode-icons, main as of 2026-08-17. */
const COMMIT = 'main'
const BASE = `https://raw.githubusercontent.com/catppuccin/vscode-icons/${COMMIT}/src/defaults`

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const OUT = join(root, 'src', 'catppuccinAssociations.json')

async function fetchText(name) {
  const url = `${BASE}/${name}`
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url} answered ${response.status}`)
  return response.text()
}

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

const [fileSource, folderSource] = await Promise.all([
  fetchText('fileIcons.ts'),
  fetchText('folderIcons.ts')
])

const fileEntries = parseEntries(fileSource)
const folderEntries = parseEntries(folderSource)

/* A parse that quietly matched nothing would write an empty table and leave the
   tree drawing the default page for every file — which looks like a design
   decision rather than a broken script. */
if (Object.keys(fileEntries).length < 200) throw new Error('file icon table looks empty')
if (Object.keys(folderEntries).length < 50) throw new Error('folder icon table looks empty')

const associations = {
  source: `catppuccin/vscode-icons@${COMMIT}`,
  license: 'MIT',
  fileNames: invert(fileEntries, 'fileNames'),
  fileExtensions: invert(fileEntries, 'fileExtensions'),
  languageIds: invert(fileEntries, 'languageIds'),
  folderNames: invert(folderEntries, 'folderNames')
}

await writeFile(OUT, `${JSON.stringify(associations, null, 0)}\n`)

const count = (table) => Object.keys(associations[table]).length
console.log(
  `wrote ${OUT}: ${count('fileNames')} names, ${count('fileExtensions')} extensions, ` +
    `${count('languageIds')} language ids, ${count('folderNames')} folder names`
)
