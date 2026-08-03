/* A "file extension → language" map. Every value is a dynamic import, so every
   language becomes its own chunk: the startup bundle does not grow, a language
   loads when the first file of its type is opened, and after that it comes from
   the module cache.

   Extending the set means adding a line here, nothing else. */
import { StreamLanguage } from '@codemirror/language'

/* Every path is written out in full: Rollup does not analyse a concatenated
   specifier, no chunk is created for it, and the import fails always — not only
   when something did not arrive over the network. */
const LEGACY = {
  toml: () => import('@codemirror/legacy-modes/mode/toml').then((m) => StreamLanguage.define(m.toml)),
  shell: () => import('@codemirror/legacy-modes/mode/shell').then((m) => StreamLanguage.define(m.shell)),
  properties: () => import('@codemirror/legacy-modes/mode/properties').then((m) => StreamLanguage.define(m.properties)),
  dockerfile: () => import('@codemirror/legacy-modes/mode/dockerfile').then((m) => StreamLanguage.define(m.dockerFile))
}

const LANGUAGES = {
  js: () => import('@codemirror/lang-javascript').then((m) => m.javascript()),
  mjs: () => import('@codemirror/lang-javascript').then((m) => m.javascript()),
  cjs: () => import('@codemirror/lang-javascript').then((m) => m.javascript()),
  jsx: () => import('@codemirror/lang-javascript').then((m) => m.javascript({ jsx: true })),
  ts: () => import('@codemirror/lang-javascript').then((m) => m.javascript({ typescript: true })),
  tsx: () => import('@codemirror/lang-javascript').then((m) => m.javascript({ jsx: true, typescript: true })),
  vue: () => import('@codemirror/lang-vue').then((m) => m.vue()),
  rs: () => import('@codemirror/lang-rust').then((m) => m.rust()),
  py: () => import('@codemirror/lang-python').then((m) => m.python()),
  go: () => import('@codemirror/lang-go').then((m) => m.go()),
  json: () => import('@codemirror/lang-json').then((m) => m.json()),
  md: () => import('@codemirror/lang-markdown').then((m) => m.markdown()),
  markdown: () => import('@codemirror/lang-markdown').then((m) => m.markdown()),
  html: () => import('@codemirror/lang-html').then((m) => m.html()),
  htm: () => import('@codemirror/lang-html').then((m) => m.html()),
  css: () => import('@codemirror/lang-css').then((m) => m.css()),
  yaml: () => import('@codemirror/lang-yaml').then((m) => m.yaml()),
  yml: () => import('@codemirror/lang-yaml').then((m) => m.yaml()),
  sql: () => import('@codemirror/lang-sql').then((m) => m.sql()),
  xml: () => import('@codemirror/lang-xml').then((m) => m.xml()),
  c: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
  h: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
  cc: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
  cpp: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
  hpp: () => import('@codemirror/lang-cpp').then((m) => m.cpp()),
  java: () => import('@codemirror/lang-java').then((m) => m.java()),
  toml: LEGACY.toml,
  sh: LEGACY.shell,
  bash: LEGACY.shell,
  zsh: LEGACY.shell,
  ini: LEGACY.properties,
  cfg: LEGACY.properties
}

/* Files whose name matters more than their extension. */
const BY_NAME = {
  dockerfile: LEGACY.dockerfile,
  makefile: LEGACY.shell
}

export async function languageFor(path) {
  const name = String(path || '').split('/').pop()?.toLowerCase() ?? ''
  const dot = name.lastIndexOf('.')
  const load = BY_NAME[name] ?? (dot > 0 ? LANGUAGES[name.slice(dot + 1)] : undefined)
  /* An unknown extension is a normal outcome: the file opens without
     highlighting. */
  if (!load) return null
  try {
    return await load()
  } catch (error) {
    /* The chunk did not arrive — offline, a broken build. The file stays plain
       text: the editor must not break because the colouring was not
       delivered. */
    console.warn('[editor] language failed to load for', path, error)
    return null
  }
}
