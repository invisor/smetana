#!/usr/bin/env node
// Validate the draft release's actual latest.json against uploaded assets and
// the signatures saved by each native build, before making the release public.
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const [manifestPath, releasePath, signaturesDir, version] = process.argv.slice(2)
if (!manifestPath || !releasePath || !signaturesDir || !version) {
  throw new Error('usage: check-linux-update-manifest.mjs <latest.json> <release.json> <signatures-dir> <version>')
}
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const release = JSON.parse(readFileSync(releasePath, 'utf8'))
if (manifest.version !== version) throw new Error(`manifest version ${manifest.version} differs from ${version}`)
if (release.tag_name !== `v${version}` || release.draft !== true) throw new Error('expected the matching draft release')
if (!Array.isArray(release.assets) || !release.assets.some((asset) => asset.name === 'latest.json')) {
  throw new Error('draft release has no latest.json asset')
}
const assets = release.assets.filter((asset) => asset.state === 'uploaded' && asset.size > 0)
const requireAsset = (pattern) => {
  const found = assets.filter((asset) => pattern.test(asset.name))
  if (found.length !== 1) throw new Error(`expected one uploaded asset matching ${pattern}, found ${found.length}`)
  return found[0]
}
requireAsset(/\.dmg$/)
requireAsset(/\.AppImage$/)
const signatures = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
  const path = join(dir, entry.name)
  return entry.isDirectory() ? signatures(path) : [path]
})
const localSignatures = signatures(signaturesDir)
const checkEntry = (key, assetPattern, signaturePattern) => {
  const entry = manifest.platforms?.[key]
  if (!entry || typeof entry.url !== 'string' || typeof entry.signature !== 'string') {
    throw new Error(`missing updater entry ${key}`)
  }
  const asset = requireAsset(assetPattern)
  if (entry.url !== asset.url) throw new Error(`${key} points to ${entry.url}, not uploaded asset ${asset.url}`)
  const encoded = entry.signature.trim()
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(encoded) || !Buffer.from(encoded, 'base64').toString('utf8').startsWith('untrusted comment:')) {
    throw new Error(`${key} has an invalid updater signature`)
  }
  const matching = localSignatures.filter((path) => signaturePattern.test(path))
  if (matching.length !== 1 || readFileSync(matching[0], 'utf8').trim() !== encoded) {
    throw new Error(`${key} signature differs from its native build`)
  }
  return { ...entry, assetName: asset.name }
}
const mac = checkEntry('darwin-aarch64', /\.app\.tar\.gz$/, /\.app\.tar\.gz\.sig$/)
const macInstaller = checkEntry('darwin-aarch64-app', /\.app\.tar\.gz$/, /\.app\.tar\.gz\.sig$/)
if (mac.url !== macInstaller.url || mac.signature !== macInstaller.signature) throw new Error('macOS installer entry differs from macOS fallback')
// Tauri 2 with `createUpdaterArtifacts: true` signs the AppImage itself and
// points the updater at it; only macOS still gets a tarball. `.AppImage.tar.gz`
// is the `v1Compatible` layout, which this app does not build.
const appimage = checkEntry('linux-x86_64-appimage', /\.AppImage$/, /\.AppImage\.sig$/)
const deb = checkEntry('linux-x86_64-deb', /\.deb$/, /\.deb\.sig$/)
const fallback = checkEntry('linux-x86_64', /\.AppImage$/, /\.AppImage\.sig$/)
if (fallback.url !== appimage.url || fallback.signature !== appimage.signature) throw new Error('Linux fallback does not select AppImage')
if (deb.url === appimage.url) throw new Error('Linux installers select the same asset')
console.log(`PASS actual updater manifest ${version}: macOS ${mac.assetName}, AppImage ${appimage.assetName}, Deb ${deb.assetName}`)
