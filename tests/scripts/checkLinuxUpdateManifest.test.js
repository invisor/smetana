import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterEach, describe, expect, it } from 'vitest'

/* scripts/check-linux-update-manifest.mjs is the last gate before a draft
   release goes public, and it is run here as the workflow runs it: a real
   process over files on disk. The fixtures follow what Tauri 2 publishes with
   `createUpdaterArtifacts: true` — a tarball for macOS, the AppImage and the
   .deb signed as they are for Linux. v0.3.7 failed to publish because the
   script expected an `.AppImage.tar.gz` that only the v1-compatible layout
   produces. */
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const SCRIPT = join(ROOT, 'scripts/check-linux-update-manifest.mjs')
const VERSION = '9.9.9'
const BASE = `https://github.com/o/r/releases/download/v${VERSION}`

const signature = (name) =>
  Buffer.from(`untrusted comment: signature for ${name}\nRWQ${name}\n`).toString('base64')

let dir
afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true })
  dir = undefined
})

function run({ assets, platforms, localSignatures }) {
  dir = mkdtempSync(join(tmpdir(), 'manifest-check-'))
  const sigDir = join(dir, 'signatures')
  for (const [path, value] of Object.entries(localSignatures)) {
    mkdirSync(dirname(join(sigDir, path)), { recursive: true })
    writeFileSync(join(sigDir, path), value)
  }
  writeFileSync(join(dir, 'latest.json'), JSON.stringify({ version: VERSION, platforms }))
  writeFileSync(
    join(dir, 'release.json'),
    JSON.stringify({
      tag_name: `v${VERSION}`,
      draft: true,
      assets: ['latest.json', ...assets].map((name) => ({
        name,
        url: `${BASE}/${name}`,
        state: 'uploaded',
        size: 1,
      })),
    })
  )
  return execFileSync(
    process.execPath,
    [SCRIPT, join(dir, 'latest.json'), join(dir, 'release.json'), sigDir, VERSION],
    { encoding: 'utf8', stdio: 'pipe' }
  )
}

const MAC = 'Smetana_aarch64.app.tar.gz'
const DMG = `Smetana_${VERSION}_aarch64.dmg`
const APPIMAGE = `Smetana_${VERSION}_amd64.AppImage`
const DEB = `Smetana_${VERSION}_amd64.deb`

function tauri2Release() {
  const entry = (name) => ({ url: `${BASE}/${name}`, signature: signature(name) })
  return {
    assets: [MAC, DMG, APPIMAGE, DEB],
    platforms: {
      'darwin-aarch64': entry(MAC),
      'darwin-aarch64-app': entry(MAC),
      'linux-x86_64': entry(APPIMAGE),
      'linux-x86_64-appimage': entry(APPIMAGE),
      'linux-x86_64-deb': entry(DEB),
    },
    localSignatures: {
      [`macos/${MAC}.sig`]: signature(MAC),
      [`appimage/${APPIMAGE}.sig`]: signature(APPIMAGE),
      [`deb/${DEB}.sig`]: signature(DEB),
    },
  }
}

describe('the final check of the draft release', () => {
  it('passes the layout Tauri 2 publishes, where the AppImage itself is the update', () => {
    expect(run(tauri2Release())).toContain(`AppImage ${APPIMAGE}`)
  })

  it('refuses an AppImage entry whose signature is not the one its build saved', () => {
    const release = tauri2Release()
    release.localSignatures[`appimage/${APPIMAGE}.sig`] = signature('something else')
    expect(() => run(release)).toThrow(/linux-x86_64-appimage signature differs/)
  })

  it('refuses a Linux fallback that does not select the AppImage', () => {
    const release = tauri2Release()
    release.platforms['linux-x86_64'] = release.platforms['linux-x86_64-deb']
    expect(() => run(release)).toThrow(/linux-x86_64 points to/)
  })
})

describe('the release workflow', () => {
  const workflow = readFileSync(join(ROOT, '.github/workflows/release.yml'), 'utf8')

  it('saves the signature Tauri 2 writes beside the AppImage, not a tarball one', () => {
    expect(workflow).toContain('bundle/appimage/*.AppImage.sig')
    expect(workflow).not.toContain('.AppImage.tar.gz')
  })
})
