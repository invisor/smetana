import { describe, expect, it } from 'vitest'
import { checkNewName } from '../../../src/components/files/newEntry.js'

describe('checkNewName', () => {
  it('hands an ordinary name straight through', () => {
    expect(checkNewName('main.rs')).toEqual({ verdict: 'make', name: 'main.rs' })
    expect(checkNewName('.gitignore')).toEqual({ verdict: 'make', name: '.gitignore' })
    expect(checkNewName('..hidden')).toEqual({ verdict: 'make', name: '..hidden' })
  })

  it('takes the spaces off the ends, which nobody meant to type', () => {
    expect(checkNewName('  notes.md  ')).toEqual({ verdict: 'make', name: 'notes.md' })
  })

  it('keeps the spaces inside a name, which somebody did', () => {
    expect(checkNewName('release notes.md')).toMatchObject({ verdict: 'make', name: 'release notes.md' })
  })

  it('answers an empty field with nothing rather than a refusal', () => {
    // Enter on a field somebody has changed their mind about is Esc by another
    // route: the draft goes away and nothing is said.
    expect(checkNewName('')).toEqual({ verdict: 'nothing', name: '' })
    expect(checkNewName()).toEqual({ verdict: 'nothing', name: '' })
  })

  it('answers a field of spaces the same way, since it looks exactly as empty', () => {
    expect(checkNewName('   ')).toEqual({ verdict: 'nothing', name: '' })
  })

  it('refuses the two names that already mean a directory', () => {
    expect(checkNewName('.')).toMatchObject({ verdict: 'refused' })
    expect(checkNewName('..')).toMatchObject({ verdict: 'refused' })
    expect(checkNewName(' .. ')).toMatchObject({ verdict: 'refused' })
  })

  it('refuses a path where a name was asked for', () => {
    // Making `a/b.js` in one keystroke is what VS Code does and is deliberately
    // out of scope, so a separator is a refusal rather than something to split
    // on. Both separators, because a webview may hand over either.
    expect(checkNewName('a/b.js')).toMatchObject({ verdict: 'refused' })
    expect(checkNewName('a\\b.js')).toMatchObject({ verdict: 'refused' })
    expect(checkNewName('/etc/passwd')).toMatchObject({ verdict: 'refused' })
  })

  it('carries the name through a refusal, for whoever writes the sentence', () => {
    expect(checkNewName(' a/b.js ').name).toBe('a/b.js')
  })
})
