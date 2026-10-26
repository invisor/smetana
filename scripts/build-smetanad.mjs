#!/usr/bin/env node
/* Builds the headless server and puts it into src-tauri/binaries under the name Tauri expects of an
   external binary: smetanad-<target-triple>[.exe], the way fetch-bd.mjs lays out bd.

   The desktop app needs it: `src-tauri/tauri.conf.json` lists it in `bundle.externalBin`, and
   Tauri's build script refuses a missing one, so `cargo test` in `src-tauri` wants it laid out too.
   `npm run tauri dev` and `npm run tauri build` run this first (`build.beforeDevCommand` and
   `build.beforeBuildCommand`), and the release workflow runs it as a step of its own.

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
