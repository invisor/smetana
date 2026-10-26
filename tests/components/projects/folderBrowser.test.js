import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import {
  cloneName,
  cloneProblem,
  listingNote,
  steps,
  validCloneUrl,
  validFolderName
} from '../../../src/components/projects/folderBrowser.js'

describe('what a clone address may be', () => {
  it('lets through the forms git is given', () => {
    for (const url of [
      'https://github.com/team/app.git',
      'http://host/app',
      'ssh://git@host/team/app.git',
      'git://host/app.git',
      'git@github.com:team/app.git',
      '  https://host/app.git  '
    ]) {
      expect(validCloneUrl(url), url).toBe(true)
    }
  })

  it('refuses options, command transports, local paths and anything with a space in it', () => {
    for (const url of [
      '',
      '-oProxyCommand=evil',
      '--upload-pack=evil',
      'ext::sh -c evil',
      'ext::sh%20-c%20evil',
      'fd::17',
      'file:///etc',
      '/etc/passwd',
      '../x',
      'https://host/a b',
      'C:\\repo',
      null
    ]) {
      expect(validCloneUrl(url), String(url)).toBe(false)
    }
  })

  it('is never more permissive than the Rust rule it mirrors', () => {
    // The refusals that rule's own test lists must be refused here too: read them out of the source.
    const rust = readFileSync('crates/core/src/placement.rs', 'utf8')
    const block = rust.slice(rust.indexOf('fn only_real_clone_addresses_are_let_through'))
    const refused = block.slice(block.indexOf('for bad in ['), block.indexOf('] {', block.indexOf('for bad in [')))
    const examples = [...refused.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((match) => JSON.parse(`"${match[1]}"`))
    expect(examples.length).toBeGreaterThan(5)
    for (const url of examples) expect(validCloneUrl(url), JSON.stringify(url)).toBe(false)
  })
})

describe('folder names', () => {
  it('are one plain segment', () => {
    for (const name of ['app', 'my app', 'app-2', 'a.b']) expect(validFolderName(name), name).toBe(true)
    for (const name of ['', '..', '.hidden', '-x', 'a/b', 'a\\b', 'a\nb', 'a\u0000b']) {
      expect(validFolderName(name), JSON.stringify(name)).toBe(false)
    }
  })

  it('come from the last segment of the address', () => {
    expect(cloneName('https://host/team/app.git')).toBe('app')
    expect(cloneName('git@host:team/app.git')).toBe('app')
    expect(cloneName('https://host/team/app/')).toBe('app')
    expect(cloneName('')).toBe(null)
  })
})

describe('what the clone form says', () => {
  it('says nothing about an empty field and names the problem otherwise', () => {
    expect(cloneProblem('', '')).toBe(null)
    expect(cloneProblem('https://host/app.git', '')).toBe(null)
    expect(cloneProblem('ext::sh -c x', '')).toMatch(/https/)
    expect(cloneProblem('https://host/app.git', '../x')).toMatch(/slashes/)
    expect(cloneProblem('https://host/', '')).toBe(null)
  })
})

describe('the path bar', () => {
  it('starts at the root, called Home, and gives every step the way back to it', () => {
    expect(steps('/home/dev', '/home/dev/work/app')).toEqual([
      { name: 'Home', path: '/home/dev' },
      { name: 'work', path: '/home/dev/work' },
      { name: 'app', path: '/home/dev/work/app' }
    ])
  })

  it('is only the root at the root, or for a path that is not under it', () => {
    expect(steps('/home/dev', '/home/dev')).toEqual([{ name: 'Home', path: '/home/dev' }])
    expect(steps('/home/dev', '/etc')).toEqual([{ name: 'Home', path: '/home/dev' }])
    expect(steps('/home/dev', '/home/devx/a')).toEqual([{ name: 'Home', path: '/home/dev' }])
    expect(steps(null, '/x')).toEqual([])
  })
})

describe('the line under the bar', () => {
  it('names a repository, a tracked one, and a cut list', () => {
    expect(listingNote(null)).toBe('')
    expect(listingNote({ repo: false })).toBe('')
    expect(listingNote({ repo: true, tracker: false })).toBe('A Git repository')
    expect(listingNote({ repo: true, tracker: true, truncated: true })).toBe(
      'A Git repository with a tracker - Only the first part of a long list is shown'
    )
  })
})
