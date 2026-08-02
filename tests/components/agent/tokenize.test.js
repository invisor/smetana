import { describe, expect, it } from 'vitest'
import { tokenize } from '../../../src/components/agent/tokenize.js'

/* Токенайзер отдаёт [{ txt, v }], где v — имя CSS-переменной или null.
   Удобнее проверять склейку: она заодно доказывает, что ни один символ не
   потерян и не задвоен. */
const joined = (line) => tokenize(line).map((token) => token.txt).join('')
const varOf = (line, txt) => tokenize(line).find((token) => token.txt === txt)?.v

describe('tokenize', () => {
  it('ничего не теряет и не задваивает', () => {
    const line = 'let x = foo(1, "два"); // хвост'
    expect(joined(line)).toBe(line)
  })

  it('пустая строка даёт пустой список', () => {
    expect(tokenize('')).toEqual([])
  })

  it('комментарий двумя косыми съедает остаток строки', () => {
    expect(tokenize('// весь остаток')).toEqual([{ txt: '// весь остаток', v: '--syn-comment' }])
  })

  it('комментарий решёткой тоже', () => {
    expect(tokenize('# и этот')).toEqual([{ txt: '# и этот', v: '--syn-comment' }])
  })

  it('строки в обеих кавычках', () => {
    expect(varOf('"текст"', '"текст"')).toBe('--syn-string')
    expect(varOf("'текст'", "'текст'")).toBe('--syn-string')
  })

  it('незакрытая строка не подвешивает разбор', () => {
    expect(joined('x = "не закрыта')).toBe('x = "не закрыта')
    expect(varOf('x = "не закрыта', '"не закрыта')).toBe('--syn-string')
  })

  it('число узнаётся вместе с суффиксом', () => {
    expect(varOf('let a = 42u32', '42u32')).toBe('--syn-number')
  })

  it('имя перед скобкой — вызов функции', () => {
    expect(varOf('foo(1)', 'foo')).toBe('--syn-function')
  })

  it('имя с большой буквы — тип', () => {
    expect(varOf('let a: String = x', 'String')).toBe('--syn-type')
  })

  it('ключевые слова узнаются', () => {
    expect(varOf('let x = 1', 'let')).toBe('--syn-keyword')
    expect(varOf('async fn go()', 'async')).toBe('--syn-keyword')
    expect(varOf('return true', 'true')).toBe('--syn-keyword')
  })

  it('обычное имя остаётся без оформления', () => {
    expect(varOf('let счётчик = 1', 'let')).toBe('--syn-keyword')
    expect(tokenize('foo').find((t) => t.txt === 'foo').v).toBe(null)
  })

  it('пунктуация и операторы разведены', () => {
    expect(varOf('a(b);', ';')).toBe('--syn-punctuation')
    expect(varOf('a >= b', '>=')).toBe('--syn-operator')
  })
})
