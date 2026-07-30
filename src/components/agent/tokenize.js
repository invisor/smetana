/* A deliberately small highlighter: enough to make an agent's code excerpt
   readable in a chat bubble. Real file viewing goes through CodeMirror 6,
   which reads the same --syn-* tokens. */
const KW = /\b(fn|let|mut|const|pub|use|struct|enum|impl|match|if|else|return|async|await|function|export|import|class|new|for|while|true|false|null|None|Some|Ok|Err)\b/

const RULES = [
  [/^\/\/.*/, '--syn-comment'],
  [/^#.*/, '--syn-comment'],
  [/^"(?:[^"\\]|\\.)*"?/, '--syn-string'],
  [/^'(?:[^'\\]|\\.)*'?/, '--syn-string'],
  [/^\b\d[\w.]*/, '--syn-number'],
  [/^[A-Za-z_][\w]*(?=\()/, '--syn-function'],
  [/^\b[A-Z][A-Za-z0-9_]*/, '--syn-type'],
  [/^[A-Za-z_][\w]*/, null],
  [/^[{}()[\];,.]/, '--syn-punctuation'],
  [/^[=+\-*/<>!&|:?%]+/, '--syn-operator'],
  [/^\s+/, null],
  [/^./, null]
]

export function tokenize(line) {
  const out = []
  let rest = line
  while (rest.length) {
    let hit = null
    for (const [re, v] of RULES) {
      const m = rest.match(re)
      if (m && m[0]) {
        hit = [m[0], v]
        break
      }
    }
    if (!hit) break
    let [txt, v] = hit
    if (v === null && KW.test(txt) && txt.match(/^[A-Za-z_]/)) v = '--syn-keyword'
    out.push({ txt, v })
    rest = rest.slice(txt.length)
  }
  return out
}
