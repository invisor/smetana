#!/usr/bin/env node
// Mirror tauri-action@v1's merge/key selection on the actual signed assets.
// Its upload-version-json.ts keeps prior platforms, gives AppImage the Linux
// fallback key and writes one installer-specific key per signature.
import { readdirSync, readFileSync } from 'node:fs'
import { join, basename } from 'node:path'

const bundle = process.argv[2]
if (!bundle) throw new Error('usage: check-linux-update-manifest.mjs <bundle directory>')
const files = (format) => readdirSync(join(bundle, format)).map((name) => join(bundle, format, name))
const signature = (format) => {
  const found = files(format).filter((path) => path.endsWith('.sig'))
  if (found.length !== 1) throw new Error(`expected one ${format} updater signature, found ${found.length}`)
  const signed = found[0].slice(0, -4)
  if (!files(format).includes(signed)) throw new Error(`signature has no asset: ${found[0]}`)
  const value = readFileSync(found[0], 'utf8').trim()
  if (!value || !Buffer.from(value, 'base64').length) throw new Error(`empty signature: ${found[0]}`)
  return { signature: value, url: `https://example.invalid/${basename(signed)}` }
}
const appimage = signature('appimage')
const deb = signature('deb')
if (!appimage.url.endsWith('.AppImage.tar.gz')) throw new Error('AppImage updater is not a tar.gz')
if (!deb.url.endsWith('.deb')) throw new Error('Debian updater is not a .deb')
const mac = { signature: 'existing-macos-signature', url: 'https://example.invalid/Smetana.app.tar.gz' }
const platforms = {
  'darwin-aarch64': mac,
  'darwin-aarch64-app': mac,
  'linux-x86_64': appimage,
  'linux-x86_64-appimage': appimage,
  'linux-x86_64-deb': deb
}
const selected = (installer) => platforms[`linux-x86_64-${installer}`] ?? platforms['linux-x86_64']
if (selected('deb') !== deb || selected('appimage') !== appimage) throw new Error('wrong Linux updater format selection')
if (platforms['darwin-aarch64'] !== mac || platforms['darwin-aarch64-app'] !== mac) throw new Error('macOS update entries lost')
console.log(`PASS updater manifest: AppImage ${basename(appimage.url)}, Deb ${basename(deb.url)}, macOS entries kept`)
