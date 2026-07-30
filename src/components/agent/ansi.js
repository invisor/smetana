/* Minimal SGR parser: enough for agent output (colour, bright, bold, dim, reset).
   Agents are console processes, so their output arrives escape-coloured; the 16
   ANSI tokens are tuned per theme against --editor-bg, not lifted from xterm. */
const BASE = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white']

export function parseAnsi(input) {
  const out = []
  let fg = null
  let bold = false
  let dim = false
  const re = /\u001b\[([0-9;]*)m/g
  let last = 0
  let m
  const push = (t) => {
    if (t) out.push({ text: t, color: fg, bold, dim })
  }
  while ((m = re.exec(input))) {
    push(input.slice(last, m.index))
    last = re.lastIndex
    for (const code of m[1].split(';')) {
      const n = parseInt(code || '0', 10)
      if (n === 0) {
        fg = null
        bold = false
        dim = false
      } else if (n === 1) bold = true
      else if (n === 2) dim = true
      else if (n === 22) {
        bold = false
        dim = false
      } else if (n >= 30 && n <= 37) fg = `var(--ansi-${BASE[n - 30]})`
      else if (n >= 90 && n <= 97) fg = `var(--ansi-bright-${BASE[n - 90]})`
      else if (n === 39) fg = null
    }
  }
  push(input.slice(last))
  return out
}
