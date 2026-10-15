/* A terminal session's output as text a phone can read.

   `terminal_attach` answers the session's ring - the bytes the program wrote, base64 - and the
   phone shows a terminal session read-only: no input, no cursor, no colours. A programme that
   draws a screen (an agent's own interface) writes escape sequences to move about that screen,
   and nothing here pretends to replay them. What comes out is the plain text that was written,
   the last lines of it, which is what a person glancing at their phone wants to know.

   Pure: the decoding takes a string and the cleaning takes a string. */

/* base64 -> UTF-8 text. `atob` is in every webview this runs in. A damaged ring is read as far
   as it goes; text is never the reason a screen fails. */
export function decodeBase64(data) {
  try {
    const binary = atob(String(data ?? ''))
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0))
    return new TextDecoder('utf-8').decode(bytes)
  } catch {
    return ''
  }
}

// ESC [ ... final byte; ESC ] ... BEL or ESC \; ESC followed by one character.
const CSI = /\u001b\[[0-9;?<=>!]*[ -/]*[@-~]/g
const OSC = /\u001b\][^\u0007\u001b]*(?:\u0007|\u001b\\)/g
const ESC = /\u001b[@-Z\\-_()#][0-9A-Za-z]?/g
const CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g

/* Escape sequences go, a bare carriage return rewinds its line (what a progress bar does, so
   only its last state is kept) and the other control characters go. */
export function plainText(text) {
  const cleaned = String(text ?? '')
    .replace(OSC, '')
    .replace(CSI, '')
    .replace(ESC, '')
    .replace(/\r\n/g, '\n')
  return cleaned
    .split('\n')
    .map((line) => line.split('\r').reverse().find((part) => part !== '') ?? '')
    .join('\n')
    .replace(CONTROL, '')
}

export const LAST_LINES = 200

/* The last `max` lines, with the blank tail the cursor left removed. */
export function lastLines(text, max = LAST_LINES) {
  const lines = plainText(text).split('\n').map((line) => line.replace(/\s+$/, ''))
  while (lines.length && lines[lines.length - 1] === '') lines.pop()
  return lines.slice(-max)
}

/* From a `terminal_attach` reply to the lines to draw. */
export const linesOf = (reply, max = LAST_LINES) => lastLines(decodeBase64(reply?.data), max)
