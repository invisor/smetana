import { describe, expect, it } from 'vitest'
import { tokenize } from '../../../src/components/agent/tokenize.js'

/* The tokenizer returns [{ txt, v }], where v is a CSS variable's name or null.
   Checking the rejoined string is more convenient: it also proves that not a
   single character was lost or doubled. */
const joined = (line) => tokenize(line).map((token) => token.txt).join('')
const varOf = (line, txt) => tokenize(line).find((token) => token.txt === txt)?.v

describe('tokenize', () => {
  it('loses nothing and doubles nothing', () => {
    const line = 'let x = foo(1, "two"); // a tail'
    expect(joined(line)).toBe(line)
  })

  it('an empty line gives an empty list', () => {
    expect(tokenize('')).toEqual([])
  })

  it('a double-slash comment eats the rest of the line', () => {
    expect(tokenize('// all the rest')).toEqual([{ txt: '// all the rest', v: '--syn-comment' }])
  })

  it('a hash comment does too', () => {
    expect(tokenize('# this one as well')).toEqual([{ txt: '# this one as well', v: '--syn-comment' }])
  })

  it('strings in both kinds of quotes', () => {
    expect(varOf('"text"', '"text"')).toBe('--syn-string')
    expect(varOf("'text'", "'text'")).toBe('--syn-string')
  })

  it('an unclosed string does not hang the parse', () => {
    expect(joined('x = "not closed')).toBe('x = "not closed')
    expect(varOf('x = "not closed', '"not closed')).toBe('--syn-string')
  })

  it('a number is recognised together with its suffix', () => {
    expect(varOf('let a = 42u32', '42u32')).toBe('--syn-number')
  })

  it('a name before a bracket is a function call', () => {
    expect(varOf('foo(1)', 'foo')).toBe('--syn-function')
  })

  it('a capitalised name is a type', () => {
    expect(varOf('let a: String = x', 'String')).toBe('--syn-type')
  })

  it('keywords are recognised', () => {
    expect(varOf('let x = 1', 'let')).toBe('--syn-keyword')
    expect(varOf('async fn go()', 'async')).toBe('--syn-keyword')
    expect(varOf('return true', 'true')).toBe('--syn-keyword')
  })

  it('a plain name stays unstyled', () => {
    expect(varOf('let counter = 1', 'let')).toBe('--syn-keyword')
    expect(tokenize('foo').find((t) => t.txt === 'foo').v).toBe(null)
  })

  it('punctuation and operators are told apart', () => {
    expect(varOf('a(b);', ';')).toBe('--syn-punctuation')
    expect(varOf('a >= b', '>=')).toBe('--syn-operator')
  })
})
