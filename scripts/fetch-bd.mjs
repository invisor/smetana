#!/usr/bin/env node
/* Puts the released bd binary into src-tauri/binaries under the name Tauri
   expects: bd-<target-triple>[.exe]. The binary is 128 MB and is not committed
   to git.

   A Homebrew build of bd is not portable — it links against icu4c from
   /opt/homebrew. The official release depends only on system libraries, so that
   is the one we take. */
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

/* This is the authority. A release tag can be re-uploaded: a substituted
   archive together with a substituted checksums.txt would agree with itself and
   pass the check. And the app runs this binary with the user's permissions,
   while the whole idea of being self-contained rests on the version being
   pinned. So the expected sums sit next to BD_VERSION, in git, and change only
   together with it. The release's checksums.txt stays as a cross-check: it
   catches a typo in this table and a table that has drifted out of step with
   the version.

   Taken from https://github.com/gastownhall/beads/releases/download/v1.1.2/checksums.txt
   2026-07-31. */
const SHA256_BY_ASSET = {
  [`beads_${BD_VERSION}_darwin_arm64.tar.gz`]:
    '9b0137a83a2afd343e2abd2a506be72ea032721000f76669c2cf81729e78501d',
  [`beads_${BD_VERSION}_darwin_amd64.tar.gz`]:
    '0e94de9319c9d66cb7e0038bb17ebaf5dd2fe669e366a4b9153528b474a1a8f6',
  [`beads_${BD_VERSION}_linux_arm64.tar.gz`]:
    'a134015faf4be0a43f8681a8d602eaf0b7c255c957f09d3c933257c8c92fdd10',
  [`beads_${BD_VERSION}_linux_amd64.tar.gz`]:
    'a72d71ed374955dc9f83a0f90b54bd7b6a0016709dd1676ae2e368651ed401c2',
  [`beads_${BD_VERSION}_windows_arm64.zip`]:
    'a4e8d717d28e4338113eff2d8aeb560af947e726c58089d39e75074e7a31244f',
  [`beads_${BD_VERSION}_windows_amd64.zip`]:
    '4591b07bf82b3203a1dc7db17a7e4962d86338e6c3d34a8a857cc11a57f9c159'
}

/* postinstall has no right to fail npm install. Somebody who came to fix a
   component needs only npm run dev and ?view=gallery — bd takes no part there
   at all, mockBackend stands in for it. Demanding a Rust toolchain and 43 MB
   over the network for that is not on. So postinstall calls this script with
   --optional: if it fails, we warn and exit zero. An explicit run
   (npm run fetch-bd) and CI have to fail: there a missing binary is a real
   breakage. */
const OPTIONAL = process.argv.includes('--optional') && !process.env.CI

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = join(root, 'src-tauri', 'binaries')

const hostTriple = () =>
  execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8' }).trim()

const targetPath = (triple) =>
  join(outDir, triple.includes('windows') ? `bd-${triple}.exe` : `bd-${triple}`)

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
  if (!asset) throw new Error(`no bd release for ${triple}`)

  const windows = triple.includes('windows')
  const target = targetPath(triple)

  const work = await mkdtemp(join(tmpdir(), 'fetch-bd-'))
  try {
    const archive = join(work, asset)
    console.log(`↓ ${asset}`)
    await download(`${BASE}/${asset}`, archive)

    // The main check is against the committed sum.
    const expected = SHA256_BY_ASSET[asset]
    if (!expected) throw new Error(`no expected sha256 for ${asset}`)
    const actual = createHash('sha256').update(await readFile(archive)).digest('hex')
    if (actual !== expected) throw new Error(`sha256 mismatch: ${actual} instead of ${expected}`)

    // The cross-check: a mismatch here means the table in this file has drifted
    // out of step with the release, not that the archive was damaged on the way.
    const published = sums.get(asset)
    if (!published) throw new Error(`${asset} is missing from the release's checksums.txt`)
    if (published !== expected) {
      throw new Error(
        `the release's checksums.txt disagrees with the expected sum: ${published} instead of ${expected}`
      )
    }

    execFileSync('tar', ['-xf', archive, '-C', work])
    await mkdir(outDir, { recursive: true })
    await copyFile(join(work, windows ? 'bd.exe' : 'bd'), target)
    await chmod(target, 0o755)
    console.log(`✓ ${target}`)
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

async function main() {
  const triples = process.argv.includes('--all') ? Object.keys(ASSET_BY_TRIPLE) : [hostTriple()]
  const missing = []
  for (const triple of triples) {
    if (existsSync(targetPath(triple))) console.log(`✓ ${triple} is already in place`)
    else missing.push(triple)
  }

  /* We go to the network only if there is really something to download:
     postinstall on a machine that already has the binary must work offline. */
  if (missing.length) {
    const sums = await checksums()
    for (const triple of missing) await install(triple, sums)
  }
}

try {
  await main()
} catch (e) {
  if (!OPTIONAL) {
    console.error(`fetch-bd: ${e.message}`)
    process.exit(1)
  }
  console.warn(`fetch-bd: ${e.message}`)
  console.warn('bd is not installed — this is only needed for a Tauri build.')
  console.warn('npm run dev and ?view=gallery work without it; when you need it: npm run fetch-bd')
}
