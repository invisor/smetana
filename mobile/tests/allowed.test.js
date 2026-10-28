import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { ALLOWED_COMMANDS, ALLOWED_EVENTS, isAllowedCommand, isAllowedEvent } from '../src/allowed.js'

const here = (path) => fileURLToPath(new URL(path, import.meta.url))
const bridgeRs = readFileSync(here('../../crates/remote-protocol/src/bridge.rs'), 'utf8')

const listIn = (name) => {
  const body = new RegExp(`${name}: &\\[&str\\] = &\\[([^\\]]*)\\]`).exec(bridgeRs)
  expect(body, `${name} in bridge.rs`).not.toBeNull()
  return [...body[1].matchAll(/"([^"]+)"/g)].map((match) => match[1])
}

describe('the phone\'s copy of the allowlist', () => {
  it('is the desktop bridge\'s list of commands, in the same order', () => {
    expect(ALLOWED_COMMANDS).toEqual(listIn('ALLOWED_COMMANDS'))
  })

  it('is the desktop bridge\'s list of events, in the same order', () => {
    expect(ALLOWED_EVENTS).toEqual(listIn('ALLOWED_EVENTS'))
  })

  it('has events with a colon and commands without, which is how one list can carry both', () => {
    expect(ALLOWED_COMMANDS.some((name) => name.includes(':'))).toBe(false)
    expect(ALLOWED_EVENTS.every((name) => name.includes(':'))).toBe(true)
  })

  it('refuses what is not on it, writes and files and git among them', () => {
    for (const name of ['tracker_close', 'files_write', 'vcs_status', 'settings_save', 'terminal_write', 'terminal_create', 'run_release', 'attachment_reopen']) {
      expect(isAllowedCommand(name), name).toBe(false)
    }
    expect(isAllowedEvent('terminal:output')).toBe(false)
    expect(isAllowedEvent('crew:tree')).toBe(true)
    expect(isAllowedEvent('remote:devices')).toBe(false)
    expect(isAllowedCommand('run_start')).toBe(true)
    expect(isAllowedEvent('run:state')).toBe(true)
  })
})

/* The commands of the Rust core itself (mobile/src-tauri/src/lib.rs), which are not the
   desktop's and never cross the bridge. */
const CORE = ['bridge_invoke', 'bridge_listen', 'bridge_unlisten', 'bridge_state', 'take_push_target', 'pair', 'connect', 'unpair']

function sources(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sources(path)
    return /\.(js|vue)$/.test(name) ? [path] : []
  })
}

describe('what the phone\'s own files ask for', () => {
  const root = here('../src')
  const named = sources(root).flatMap((file) =>
    [...readFileSync(file, 'utf8').matchAll(/nvoke\(\s*'([a-z_]+)'/g)].map((match) => ({
      file: relative(root, file),
      name: match[1]
    }))
  )

  it('finds the calls this audit is about', () => {
    expect(named.length).toBeGreaterThan(3)
    expect(named.map((call) => call.name)).toContain('settings_load')
    expect(named.map((call) => call.name)).toContain('terminal_attach')
  })

  it('names nothing off the list except the core\'s own commands', () => {
    const stray = named.filter((call) => !isAllowedCommand(call.name) && !CORE.includes(call.name))
    expect(stray).toEqual([])
  })

  it('listens to nothing off the list', () => {
    const listened = sources(root).flatMap((file) =>
      [...readFileSync(file, 'utf8').matchAll(/(?:listen|once)\(\s*'([a-z:_-]+)'/g)].map((match) => match[1])
    )
    const core = ['bridge:event', 'bridge:status', 'push:target']
    expect(listened.filter((name) => !isAllowedEvent(name) && !core.includes(name))).toEqual([])
  })
})
