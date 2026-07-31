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

/* Здесь авторитет. Тег в релизе можно перезалить: подменённый архив вместе с
   подменённым checksums.txt сойдётся сам с собой и проверку пройдёт. А
   приложение запускает этот бинарник с правами пользователя, и вся идея
   самодостаточности держится на том, что версия зафиксирована. Поэтому
   ожидаемые суммы лежат рядом с BD_VERSION, в git, и меняются только вместе с
   ней. checksums.txt из релиза остаётся перекрёстной сверкой: он ловит
   опечатку в этой таблице и рассинхрон таблицы с версией.

   Снято с https://github.com/gastownhall/beads/releases/download/v1.1.2/checksums.txt
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

/* postinstall не имеет права ронять npm install. Тому, кто пришёл поправить
   компонент, нужны только npm run dev и ?view=gallery — bd там не участвует
   вовсе, за него отвечает mockBackend. Требовать ради этого тулчейн Rust и
   43 МБ по сети нельзя. Поэтому postinstall зовёт скрипт с --optional:
   сорвалось — предупреждаем и выходим нулём. Явный запуск (npm run fetch-bd)
   и CI обязаны падать: там отсутствие бинарника — настоящая поломка. */
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
  if (!asset) throw new Error(`нет релиза bd для ${triple}`)

  const windows = triple.includes('windows')
  const target = targetPath(triple)

  const work = await mkdtemp(join(tmpdir(), 'fetch-bd-'))
  try {
    const archive = join(work, asset)
    console.log(`↓ ${asset}`)
    await download(`${BASE}/${asset}`, archive)

    // Главная проверка — по вкомиченной сумме.
    const expected = SHA256_BY_ASSET[asset]
    if (!expected) throw new Error(`нет ожидаемой sha256 для ${asset}`)
    const actual = createHash('sha256').update(await readFile(archive)).digest('hex')
    if (actual !== expected) throw new Error(`sha256 не совпал: ${actual} вместо ${expected}`)

    // Перекрёстная: расхождение здесь означает, что таблица в этом файле
    // разъехалась с релизом, а не что архив побился по дороге.
    const published = sums.get(asset)
    if (!published) throw new Error(`${asset} отсутствует в checksums.txt релиза`)
    if (published !== expected) {
      throw new Error(
        `checksums.txt релиза расходится с ожидаемой суммой: ${published} вместо ${expected}`
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
    if (existsSync(targetPath(triple))) console.log(`✓ ${triple} уже на месте`)
    else missing.push(triple)
  }

  /* В сеть выходим, только если действительно есть что качать: postinstall
     на машине с уже загруженным бинарником обязан работать офлайн. */
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
  console.warn('bd не установлен — это нужно только для сборки Tauri.')
  console.warn('npm run dev и ?view=gallery работают и без него; когда понадобится: npm run fetch-bd')
}
