import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import {
  CAVEMAN_INHERIT,
  CAVEMAN_LEVELS,
  installCommand,
  installDescription,
  isLevel,
  isProjectLevel,
  levelOptions,
  offersInstall,
  projectLevelOptions,
  stateFacts,
  stateSentence
} from '../../../src/components/settings/caveman.js'

/* Rust's own copy of the ladder, read off the source rather than repeated here.
   `tests/scripts/release.test.js` reading `tauri.conf.json` is the precedent for
   a test that opens a file outside `src/`, and the reason is sharper in this
   one: a repetition here would be a fourth copy of the very list this file
   exists to keep at two. */
const MODEL_RS = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../../src-tauri/src/settings/model.rs'
)

/* `pub const CAVEMAN_LEVELS: [&str; 7] = ["off", …];` — matched over the whole
   declaration, since it is written across two lines. */
function rustLevels() {
  const source = readFileSync(MODEL_RS, 'utf8')
  const declaration = source.match(/CAVEMAN_LEVELS:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]*)\]/)
  if (!declaration) throw new Error('CAVEMAN_LEVELS was not found in settings/model.rs')
  return [...declaration[1].matchAll(/"([^"]+)"/g)].map((match) => match[1])
}

describe('the caveman levels offered on the Agents tab', () => {
  it('is a subset of what Rust accepts', () => {
    /* The obligation every doubled vocabulary in this tree carries. A level only
       the front end knows is written to state, saved, rewritten to `off` by
       `one_of` in `CavemanSettings::validate` and back as `off` at the next open
       — no error anywhere, and nothing on screen to say so. */
    const accepted = rustLevels()
    expect(accepted.length).toBeGreaterThan(0)
    for (const level of CAVEMAN_LEVELS) expect(accepted).toContain(level)
  })

  it('offers off first, and the seven rungs in the ladder\'s order', () => {
    expect(CAVEMAN_LEVELS).toEqual([
      'off',
      'lite',
      'full',
      'ultra',
      'wenyan-lite',
      'wenyan-full',
      'wenyan-ultra'
    ])
    const options = levelOptions()
    expect(options).toHaveLength(7)
    expect(options[0]).toEqual({ value: 'off', label: 'Off' })
    expect(options.at(-1)).toEqual({ value: 'wenyan-ultra', label: 'Wenyan ultra' })
  })

  it('offers the project the same seven with the override in front', () => {
    const options = projectLevelOptions()
    expect(options).toHaveLength(8)
    expect(options[0]).toEqual({ value: CAVEMAN_INHERIT, label: 'Same as all projects' })
    expect(options.slice(1)).toEqual(levelOptions())
  })

  it('hands back a fresh list, so a caller cannot sort the ladder in place', () => {
    const first = levelOptions()
    first.reverse()
    expect(levelOptions()[0].value).toBe('off')
  })

  it('knows a rung from anything else', () => {
    expect(isLevel('off')).toBe(true, 'off is a rung and not an absence')
    expect(isLevel('wenyan-ultra')).toBe(true)
    expect(isLevel('inherit')).toBe(false, 'inherit is the project ladder\'s word alone')
    expect(isLevel('loud')).toBe(false)
    expect(isLevel(null)).toBe(false)
    expect(isLevel(7)).toBe(false)
  })

  it('knows the project ladder is the same list plus inherit', () => {
    expect(isProjectLevel('inherit')).toBe(true)
    expect(isProjectLevel('lite')).toBe(true)
    expect(isProjectLevel('shout')).toBe(false)
    expect(isProjectLevel(null)).toBe(false)
  })
})

describe('what the Caveman group says about this machine', () => {
  const wired = {
    state: 'wired',
    packVersion: '2.2.0',
    detectedAgentVersion: '2.1.258 (Claude Code)',
    replacedFiles: ['/home/p/.claude/settings.json', '/home/p/.claude.json']
  }
  const bare = (state) => ({
    state,
    packVersion: null,
    detectedAgentVersion: null,
    replacedFiles: []
  })

  it('draws a sentence of its own for each of the four states', () => {
    const sentences = ['absent', 'binaries-only', 'wired', 'project-skill-only'].map((state) =>
      stateSentence(bare(state))
    )
    expect(new Set(sentences).size).toBe(4)
    for (const sentence of sentences) expect(sentence.length).toBeGreaterThan(0)
  })

  it('says it has nothing yet before a reading arrives, and for a word it has never heard', () => {
    const nothing = stateSentence(null)
    expect(nothing).toBe(stateSentence({ state: 'something-new' }))
    expect(nothing).not.toBe(stateSentence(bare('absent')))
  })

  it('shows the pack version and every replaced file where it is wired in', () => {
    const facts = stateFacts(wired)
    expect(facts).toContainEqual({ name: 'Pack version', value: '2.2.0' })
    expect(facts).toContainEqual({ name: 'Applied to', value: '2.1.258 (Claude Code)' })
    expect(facts.filter((fact) => fact.name === 'Replaced').map((fact) => fact.value)).toEqual(
      wired.replacedFiles
    )
  })

  it('shows nothing off the journal in any other state', () => {
    /* The journal survives an install that has since been unwired, so its file
       list under "installed and switched off" would claim rewrites that may
       have been put back. */
    expect(stateFacts({ ...wired, state: 'binaries-only' })).toEqual([])
    expect(stateFacts(bare('absent'))).toEqual([])
    expect(stateFacts(null)).toEqual([])
  })

  it('leaves out a fact the journal did not carry', () => {
    expect(stateFacts({ ...wired, packVersion: null, detectedAgentVersion: null })).toEqual([
      { name: 'Replaced', value: '/home/p/.claude/settings.json' },
      { name: 'Replaced', value: '/home/p/.claude.json' }
    ])
    expect(stateFacts({ state: 'wired' })).toEqual([])
  })
})

describe('the Install button', () => {
  const of = (state) => ({ state, packVersion: null, detectedAgentVersion: null, replacedFiles: [] })

  it('is offered where there is something to install or to wire in', () => {
    expect(offersInstall(of('absent'))).toBe(true)
    expect(offersInstall(of('binaries-only'))).toBe(true)
  })

  it('is not offered where it would do nothing', () => {
    expect(offersInstall(of('wired'))).toBe(false)
    expect(offersInstall(of('project-skill-only'))).toBe(false)
    expect(offersInstall(null)).toBe(false)
    expect(offersInstall(of('something-new'))).toBe(false)
  })

  it('types the install and the wiring, and nothing else', () => {
    expect(installCommand(of('absent'))).toBe(
      'npm i -g @caveman-ai/cli && caveman setup --install && caveman enable claude'
    )
    expect(installCommand(of('binaries-only'))).toBe('caveman enable claude')
    expect(installCommand(of('wired'))).toBe(null)
    expect(installCommand(null)).toBe(null)
  })

  it('never sends a download into a shell', () => {
    /* The one shape of install command this app will not put in front of
       somebody: whatever the far end serves at the moment of the press, run
       before it can be read. */
    for (const state of ['absent', 'binaries-only']) {
      const command = installCommand(of(state))
      expect(command).not.toMatch(/curl|wget/)
      expect(command).not.toMatch(/\|\s*(ba)?sh/)
    }
  })

  it('names the reason it cannot be pressed with no project open', () => {
    const shut = installDescription(of('absent'), false)
    expect(shut).toMatch(/project/i)
    expect(shut).not.toBe(installDescription(of('absent'), true))
  })

  it('says the command is typed rather than run', () => {
    for (const state of ['absent', 'binaries-only']) {
      expect(installDescription(of(state), true)).toMatch(/Enter/)
    }
  })
})
