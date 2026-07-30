#!/usr/bin/env node
/* Кладёт релизный бинарник bd в src-tauri/binaries под именем, которого ждёт Tauri:
   bd-<target-triple>[.exe]. Бинарник весит 128 МБ и в git не коммитится.

   Сборка bd из Homebrew непереносима — она линкуется на icu4c из /opt/homebrew.
   Официальный релиз зависит только от системных библиотек, поэтому берём именно его. */
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync } from 'node:fs'
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const BD_VERSION = '1.1.2'
const BASE = `https://github.com/gastownhall/beads/releases/download/v${BD_VERSION}`

const ASSET_BY_TRIPLE = {
  'aarch64-apple-darwin': `beads_${BD_VERSION}_darwin_arm64.tar.gz`,
  'x86_64-apple-darwin': `beads_${BD_VERSION}_darwin_amd64.tar.gz`,
  'aarch64-unknown-linux-gnu': `beads_${BD_VERSION}_linux_arm64.tar.gz`,
  'x86_64-unknown-linux-gnu': `beads_${BD_VERSION}_linux_amd64.tar.gz`,
  'aarch64-pc-windows-msvc': `beads_${BD_VERSION}_windows_arm64.zip`,
  'x86_64-pc-windows-msvc': `beads_${BD_VERSION}_windows_amd64.zip`
}

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = join(root, 'src-tauri', 'binaries')

const hostTriple = () =>
  execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8' }).trim()

async function download(url, dest) {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`${url} → HTTP ${res.status}`)
  await writeFile(dest, Buffer.from(await res.arrayBuffer()))
}

async function checksums() {
  const res = await fetch(`${BASE}/checksums.txt`)
  if (!res.ok) throw new Error(`checksums.txt → HTTP ${res.status}`)
  const map = new Map()
  for (const line of (await res.text()).split('\n')) {
    const [sum, name] = line.trim().split(/\s+/)
    if (sum && name) map.set(name.replace(/^\*/, ''), sum)
  }
  return map
}

async function install(triple, sums) {
  const asset = ASSET_BY_TRIPLE[triple]
  if (!asset) throw new Error(`нет релиза bd для ${triple}`)

  const windows = triple.includes('windows')
  const target = join(outDir, windows ? `bd-${triple}.exe` : `bd-${triple}`)
  if (existsSync(target)) {
    console.log(`✓ ${triple} уже на месте`)
    return
  }

  const work = await mkdtemp(join(tmpdir(), 'fetch-bd-'))
  try {
    const archive = join(work, asset)
    console.log(`↓ ${asset}`)
    await download(`${BASE}/${asset}`, archive)

    const expected = sums.get(asset)
    if (!expected) throw new Error(`${asset} отсутствует в checksums.txt`)
    const actual = createHash('sha256').update(await readFile(archive)).digest('hex')
    if (actual !== expected) throw new Error(`sha256 не совпал: ${actual} вместо ${expected}`)

    execFileSync('tar', ['-xf', archive, '-C', work])
    await mkdir(outDir, { recursive: true })
    await copyFile(join(work, windows ? 'bd.exe' : 'bd'), target)
    await chmod(target, 0o755)
    console.log(`✓ ${target}`)
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

const triples = process.argv.includes('--all') ? Object.keys(ASSET_BY_TRIPLE) : [hostTriple()]
const sums = await checksums()
for (const triple of triples) await install(triple, sums)
