import { describe, expect, it } from 'vitest'
import { decodeBase64, lastLines, linesOf, plainText } from '../src/terminalText.js'

const b64 = (text) => Buffer.from(text, 'utf8').toString('base64')
const ESC = '\u001b'

describe('a terminal session as text', () => {
  it('decodes the ring as UTF-8', () => {
    expect(decodeBase64(b64('héllo — ok'))).toBe('héllo — ok')
  })

  it('reads a damaged ring as nothing rather than failing', () => {
    expect(decodeBase64('%%% not base64')).toBe('')
    expect(decodeBase64(undefined)).toBe('')
  })

  it('removes colour and cursor sequences', () => {
    expect(plainText(`${ESC}[31mred${ESC}[0m plain ${ESC}[2K${ESC}[1;5H.`)).toBe('red plain .')
  })

  it('removes a window title and a hyperlink sequence', () => {
    expect(plainText(`${ESC}]0;title\u0007text${ESC}]8;;http://x${ESC}\\link`)).toBe('textlink')
  })

  it('keeps the last state of a line that was redrawn with a carriage return', () => {
    expect(plainText('10%\r50%\r100%\ndone')).toBe('100%\ndone')
  })

  it('treats a carriage return and line feed as one line break', () => {
    expect(plainText('a\r\nb\r\n')).toBe('a\nb\n')
  })

  it('drops the other control characters but keeps tabs', () => {
    expect(plainText('a\u0000b\u0007c\td')).toBe('abc\td')
  })

  it('keeps the last lines and drops the blank tail the cursor left', () => {
    const text = Array.from({ length: 10 }, (_, i) => `line ${i}`).join('\n') + '\n\n   \n'
    expect(lastLines(text, 3)).toEqual(['line 7', 'line 8', 'line 9'])
  })

  it('trims the end of every line', () => {
    expect(lastLines('a   \nb\t')).toEqual(['a', 'b'])
  })

  it('goes from a terminal_attach reply to lines', () => {
    expect(linesOf({ data: b64(`${ESC}[1mhi${ESC}[0m\nthere\n`), seq: 3 })).toEqual(['hi', 'there'])
    expect(linesOf(null)).toEqual([])
  })
})
