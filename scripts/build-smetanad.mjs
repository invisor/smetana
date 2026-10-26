#!/usr/bin/env node
/* Builds the headless server and puts it into src-tauri/binaries under the name Tauri expects of an
   external binary: smetanad-<target-triple>[.exe], the way fetch-bd.mjs lays out bd.

   Only the Smetana Next build needs it (`src-tauri/tauri.next.conf.json` adds it to
   `bundle.externalBin`); the default build does not name it, so `npm run tauri build` without the
   overlay never looks for this file and a contributor who only works on the release never has to
   run this.

   `--release` builds the optimised binary, which is what a bundle wants; the default is a debug
   build, which is what `tauri dev` wants and is much faster to make. */
import { execFileSync } from 'node:child_process'
import { copyFileSync, chmodSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const release = process.argv.includes('--release')
const triple = execFileSync('rustc', ['--print', 'host-tuple'], { encoding: 'utf8' }).trim()
const windows = triple.includes('windows')
const manifest = join(root, 'crates', 'smetanad', 'Cargo.toml')

execFileSync('cargo', ['build', '--manifest-path', manifest, '--bin', 'smetanad', ...(release ? ['--release'] : [])], {
  stdio: 'inherit'
})

const built = join(root, 'crates', 'smetanad', 'target', release ? 'release' : 'debug', windows ? 'smetanad.exe' : 'smetanad')
const outDir = join(root, 'src-tauri', 'binaries')
const target = join(outDir, windows ? `smetanad-${triple}.exe` : `smetanad-${triple}`)
mkdirSync(outDir, { recursive: true })
copyFileSync(built, target)
if (!windows) chmodSync(target, 0o755)
console.log(`smetanad: ${target}`)
