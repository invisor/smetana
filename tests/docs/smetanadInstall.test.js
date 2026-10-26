import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

/* The install story for the headless server is three files that have to agree with each other and
   with the binary: the systemd unit, the instructions that tell a person to copy it, and the release
   job that puts it in the archive. None of them is code, so nothing else would notice one drifting. */
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const read = (path) => readFileSync(resolve(root, path), 'utf8')

const unit = read('server/smetanad/smetanad.service')
const readme = read('server/smetanad/README.md')
const workflow = read('.github/workflows/release.yml')
const cli = read('crates/smetanad/src/cli.rs')

describe('the systemd unit', () => {
  const exec = unit.split('\n').find((line) => line.startsWith('ExecStart='))

  it('starts the server with only flags the command line has', () => {
    expect(exec).toBeTruthy()
    const run = cli.slice(cli.indexOf('Run {'), cli.indexOf('Pair {'))
    for (const flag of exec.match(/--[a-z-]+/g)) {
      const field = flag.slice(2).replace(/-/g, '_')
      expect(run, flag).toContain(`${field}:`)
    }
    expect(exec).toContain(' run ')
  })

  it('runs as somebody who is not root, because the server refuses to', () => {
    expect(unit).toMatch(/^User=(?!root)\S+/m)
  })

  it('treats a restart the server asks for as success and a stop it asks for as the end', () => {
    expect(unit).toMatch(/^Restart=on-failure/m)
    expect(unit).toMatch(/^RestartForceExitStatus=75/m)
    expect(unit).toMatch(/^SuccessExitStatus=75/m)
  })

  it('points at the places the instructions put things', () => {
    expect(exec).toContain('/opt/smetana/')
    expect(exec).toContain('/var/lib/smetana/data')
    expect(readme).toContain('/opt/smetana')
    expect(readme).toContain('/var/lib/smetana/data')
    expect(readme).toContain('smetanad.service')
  })
})

describe('the instructions', () => {
  it('name the pairing command with the kind a desktop needs', () => {
    expect(readme).toContain('smetanad pair')
    expect(readme).toContain('--kind desktop')
  })

  it('tell the person where in the app the code goes', () => {
    expect(readme).toMatch(/Settings, then Server/)
  })
})

describe('the release job', () => {
  it('packs the server for every platform and carries the unit, the instructions and bd', () => {
    expect(workflow).toContain('smetanad:')
    for (const target of ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc']) {
      expect(workflow, target).toContain(target)
    }
    expect(workflow).toContain('server/smetanad/smetanad.service')
    expect(workflow).toContain('server/smetanad/README.md')
    expect(workflow).toContain('scripts/fetch-bd.mjs')
    expect(workflow).toContain('gh release upload')
  })

  it('waits for the release the app job publishes', () => {
    const job = workflow.slice(workflow.indexOf('\n  smetanad:'))
    expect(job).toMatch(/needs: release\b/)
  })
})
