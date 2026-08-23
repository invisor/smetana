import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { STEPS, bumpVersion, parseStep, versionIn, withVersion } from '../../scripts/release.mjs'

/* The pure halves of scripts/release.mjs — what the argument means, what the
   next number is, and what the file looks like once it carries it. The rest of
   that script is git, and git is the half a test must not reach: it commits,
   pushes and tags, and a test that shelled out to it would either write to the
   working tree it is running in or push to origin. So the script keeps its
   entry point behind an `import.meta.url === argv[1]` guard, which is what
   makes importing it here safe, and everything worth checking is exported.

   `tests/` mirrors `src/`, and this directory mirrors `scripts/` instead —
   `tests/docs/claudeMd.test.js` is the precedent for a test directory that
   holds something outside `src/`. */
const CONF = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../src-tauri/tauri.conf.json'
)

describe('the version step given on the command line', () => {
  it('is a patch when there is no argument at all', () => {
    // The common case, and the reason it is the default: a default nobody has
    // to type is a default people use.
    expect(parseStep([])).toBe('patch')
  })

  it('is whichever of the three was named', () => {
    expect(parseStep(['patch'])).toBe('patch')
    expect(parseStep(['minor'])).toBe('minor')
    expect(parseStep(['major'])).toBe('major')
    expect(STEPS).toEqual(['patch', 'minor', 'major'])
  })

  it('refuses anything else, naming all three', () => {
    // Including an exact version number, which is the mistake this whole
    // script exists to make impossible: the number comes out of the conf.
    for (const bad of ['0.2.0', 'Patch', 'v1.0.0', '']) {
      expect(() => parseStep([bad])).toThrow(/patch, minor, major/)
    }
  })

  it('refuses more than one argument', () => {
    expect(() => parseStep(['minor', 'major'])).toThrow(/at most one argument/)
  })
})

describe('the next version', () => {
  it('raises the component asked for and clears the ones below it', () => {
    expect(bumpVersion('0.1.0', 'patch')).toBe('0.1.1')
    expect(bumpVersion('0.1.0', 'minor')).toBe('0.2.0')
    expect(bumpVersion('0.1.0', 'major')).toBe('1.0.0')
  })

  it('counts rather than concatenates', () => {
    // The whole arithmetic is three integers, which is why no semver
    // dependency was added; string work would answer 0.1.91 here.
    expect(bumpVersion('0.1.9', 'patch')).toBe('0.1.10')
    expect(bumpVersion('1.9.4', 'minor')).toBe('1.10.0')
    expect(bumpVersion('9.4.2', 'major')).toBe('10.0.0')
  })

  it('drops everything below the raised component', () => {
    expect(bumpVersion('2.7.3', 'minor')).toBe('2.8.0')
    expect(bumpVersion('2.7.3', 'major')).toBe('3.0.0')
  })

  it('refuses a version that is not three integers', () => {
    for (const bad of ['0.1', '0.1.0-beta.1', '1.0.0.0', 'v0.1.0', '']) {
      expect(() => bumpVersion(bad, 'patch')).toThrow(/major>\.<minor>\.<patch/)
    }
  })

  it('refuses a step it does not know, rather than falling through to patch', () => {
    expect(() => bumpVersion('0.1.0', 'release')).toThrow(/patch, minor, major/)
  })
})

describe('the version written back into the conf', () => {
  const conf = readFileSync(CONF, 'utf8')

  it('is the one the file already carries', () => {
    expect(versionIn(conf)).toMatch(/^\d+\.\d+\.\d+$/)
  })

  it('changes that number and nothing else in the file', () => {
    // The reason the field is substituted in the text rather than the parsed
    // object re-serialised: the diff a person reads before a release should be
    // one line, not a reformatted conf.
    const raised = withVersion(conf, '9.9.9')
    expect(versionIn(raised)).toBe('9.9.9')
    expect(raised.split('\n')).toHaveLength(conf.split('\n').length)
    expect(raised.replace('9.9.9', versionIn(conf))).toBe(conf)
  })

  it('leaves the updater key and the identifier alone', () => {
    // Everything the workflow's check job reads out of this file besides the
    // version. A substitution that reached one of these would publish a
    // release nobody can install as an update.
    const raised = JSON.parse(withVersion(conf, '9.9.9'))
    const before = JSON.parse(conf)
    expect(raised.identifier).toBe(before.identifier)
    expect(raised.plugins.updater).toEqual(before.plugins.updater)
  })

  it('refuses a file whose "version" is not unique, instead of guessing', () => {
    const two = '{ "version": "0.1.0", "bundle": { "version": "1.2.3" } }'
    expect(() => withVersion(two, '0.1.1')).toThrow(/exactly one "version" field/)
    expect(() => withVersion('{ "name": "smetana" }', '0.1.1')).toThrow(
      /exactly one "version" field/
    )
  })
})
