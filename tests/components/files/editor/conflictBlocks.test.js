import { describe, expect, it } from 'vitest'
import { conflictBlocks } from '../../../../src/components/files/editor/conflictBlocks.js'

/* The rule is about text, so the fixtures are written as text and split here:
   a line at a time reads as a list, and this reads as a file. */
const blocksOf = (text) => conflictBlocks(text.split('\n'))

describe('conflict blocks', () => {
  it('an ordinary file has none', () => {
    expect(blocksOf('const a = 1\nconst b = 2\n')).toEqual([])
  })

  it('the four markers of a plain block come back as 1-based line numbers', () => {
    const text = [
      'function timeout() {',
      '<<<<<<< HEAD',
      '  return 30000',
      '=======',
      '  return 5000',
      '>>>>>>> fix/timeout',
      '}'
    ].join('\n')

    expect(blocksOf(text)).toEqual([{ start: 2, base: null, separator: 4, end: 6 }])
  })

  it('a diff3 block carries its base section', () => {
    const text = [
      '<<<<<<< HEAD',
      '  return 3',
      '||||||| merged common ancestors',
      '  return 1',
      '=======',
      '  return 5',
      '>>>>>>> fix/retries'
    ].join('\n')

    expect(blocksOf(text)).toEqual([{ start: 1, base: 3, separator: 5, end: 7 }])
  })

  it('several blocks in one file come back in order', () => {
    const text = [
      '<<<<<<< HEAD',
      'a',
      '=======',
      'b',
      '>>>>>>> theirs',
      'untouched',
      '<<<<<<< HEAD',
      'c',
      '=======',
      'd',
      '>>>>>>> theirs'
    ].join('\n')

    expect(blocksOf(text)).toEqual([
      { start: 1, base: null, separator: 3, end: 5 },
      { start: 7, base: null, separator: 9, end: 11 }
    ])
  })

  it('a setext heading underline is not a conflict', () => {
    expect(blocksOf('Release notes\n=======\n\nSomething happened.\n')).toEqual([])
  })

  it('a block nothing closes is not a conflict', () => {
    expect(blocksOf('<<<<<<< HEAD\na\n=======\nb\n')).toEqual([])
  })

  it('a close with no separator above it is not a conflict', () => {
    expect(blocksOf('<<<<<<< HEAD\na\n>>>>>>> theirs\n')).toEqual([])
  })

  it('eight marker characters are not a marker', () => {
    const text = ['<<<<<<<< HEAD', 'a', '========', 'b', '>>>>>>>> theirs'].join('\n')

    expect(blocksOf(text)).toEqual([])
  })

  it('a bare marker with nothing after it counts', () => {
    expect(blocksOf('<<<<<<<\na\n=======\nb\n>>>>>>>')).toEqual([
      { start: 1, base: null, separator: 3, end: 5 }
    ])
  })

  it('a second opening before the separator starts the block over', () => {
    const text = [
      '<<<<<<< HEAD',
      'stray',
      '<<<<<<< HEAD',
      'a',
      '=======',
      'b',
      '>>>>>>> theirs'
    ].join('\n')

    expect(blocksOf(text)).toEqual([{ start: 3, base: null, separator: 5, end: 7 }])
  })

  it('a separator and a close outside any block are ordinary text', () => {
    expect(blocksOf('=======\nsome prose\n>>>>>>> theirs\n')).toEqual([])
  })

  it('a base marker outside a block is ordinary text', () => {
    expect(blocksOf('||||||| not a merge\n=======\n>>>>>>> theirs\n')).toEqual([])
  })

  it('a base marker below the separator is not taken as a base section', () => {
    const text = ['<<<<<<< HEAD', 'a', '=======', '||||||| stray', 'b', '>>>>>>> theirs'].join('\n')

    expect(blocksOf(text)).toEqual([{ start: 1, base: null, separator: 3, end: 6 }])
  })

  it('an empty file has none', () => {
    expect(conflictBlocks([])).toEqual([])
  })
})
