#!/usr/bin/env node
/* Cuts a release: one command in place of the sequence a person used to run by
   hand — raise the version, run the gates, commit, push main, tag, push the tag.

   The tag is the entire trigger. .github/workflows/release.yml listens on
   `push: tags: - 'v*'` and on nothing else, and its first job compares the tag
   with `version` in src-tauri/tauri.conf.json and refuses to build when the two
   disagree. So the way to keep them agreeing is never to type either number:
   the version is read out of the conf, one component of it is raised, and the
   tag is that result with a `v` in front. There is no argument for an exact
   version, and that absence is the point of this script rather than a gap in
   it — naming the number by hand is the one way to get it wrong.

   What this deliberately does not do: it never talks to the GitHub API and
   never creates a release. Everything after the tag is pushed belongs to the
   workflow, which builds and signs on a runner holding the secrets. A laptop
   doing the same would need the private updater key, and the point of keeping
   that key off every machine but the runner is that no laptop has it. */
import { execFileSync, spawnSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const CONF = 'src-tauri/tauri.conf.json'
const BRANCH = 'main'
const REMOTE = 'origin'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const confPath = join(root, CONF)

/* All three of the project's gates, in the order they are cheapest to fail in.
   The asymmetry is deliberate: a local run costs a couple of minutes, and
   withdrawing a release that has already been published costs far more than
   that — plugins.updater.endpoints points at /releases/latest/, so a broken
   release is what an updater sees. main is green by construction, since work
   reaches it only through a merge that ran these same three; but the commit a
   release is tagged on is a new one that no gate has ever seen.

   cargo is pointed at the manifest rather than run from inside src-tauri so
   that every gate runs with the same working directory, which is the one the
   script computed for itself. */
const GATES = [
  { label: 'npm test', command: 'npm', args: ['test'] },
  { label: 'npm run build', command: 'npm', args: ['run', 'build'] },
  {
    label: 'cargo test',
    command: 'cargo',
    args: ['test', '--manifest-path', 'src-tauri/Cargo.toml']
  }
]

export const STEPS = ['patch', 'minor', 'major']

/* No argument at all is the common case and means a patch: most releases are
   one, and a default that has to be typed is a default nobody uses. */
export function parseStep(args) {
  if (args.length === 0) return 'patch'
  if (args.length > 1) {
    throw new Error(
      `expected at most one argument, one of ${STEPS.join(', ')} — got ${args.length}`
    )
  }
  const [step] = args
  if (!STEPS.includes(step)) {
    throw new Error(`"${step}" is not a version step — expected one of ${STEPS.join(', ')}`)
  }
  return step
}

export function bumpVersion(version, step) {
  if (!STEPS.includes(step)) {
    throw new Error(`"${step}" is not a version step — expected one of ${STEPS.join(', ')}`)
  }
  const parts = /^(\d+)\.(\d+)\.(\d+)$/.exec(version)
  if (!parts) {
    throw new Error(`version "${version}" in ${CONF} is not <major>.<minor>.<patch>`)
  }
  const [major, minor, patch] = parts.slice(1).map(Number)
  if (step === 'major') return `${major + 1}.0.0`
  if (step === 'minor') return `${major}.${minor + 1}.0`
  return `${major}.${minor}.${patch + 1}`
}

const VERSION_FIELD = /("version"\s*:\s*")([^"]*)(")/

export function versionIn(text) {
  const { version } = JSON.parse(text)
  if (typeof version !== 'string') throw new Error(`${CONF} carries no "version" string`)
  return version
}

/* Read through JSON.parse and written back through a substitution over the
   text. That is not two habits mixed but the only pair that works: parsing
   answers what the version is using the file's own grammar, while
   re-serialising the parsed object would rewrite every line of a hand-formatted
   conf for the sake of one number, and the diff a person reviews before a
   release should be that one number. The substitution is safe only while the
   field is unique in the file, so uniqueness is checked rather than assumed. */
export function withVersion(text, next) {
  const occurrences = text.match(/"version"\s*:\s*"/g) ?? []
  if (occurrences.length !== 1) {
    throw new Error(
      `expected exactly one "version" field in ${CONF}, found ${occurrences.length} — ` +
        'raise the version by hand and teach this script which one is the app\'s'
    )
  }
  return text.replace(VERSION_FIELD, `$1${next}$3`)
}

/* Trailing whitespace only, never leading: `git status --porcelain` puts the
   staged column first, so trimming the front would turn " M README.md" into
   "M README.md" on the first line alone and quietly restage it for a reader. */
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trimEnd()

// The ones whose own output is worth watching: a fetch, a push, the commit.
const gitLive = (...args) => {
  const { status, error } = spawnSync('git', args, { cwd: root, stdio: 'inherit' })
  if (error) throw new Error(`git ${args[0]} could not be started: ${error.message}`)
  if (status !== 0) throw new Error(`git ${args.join(' ')} failed`)
}

const commits = (n) => `${n} commit${n === 1 ? '' : 's'}`

/* Every refusal names its own reason. "cannot release" gives somebody nothing
   to act on, and each of these is fixed by one command once you know which of
   them it is. Nothing here has touched a file yet, which is what makes a
   refusal free: the tree afterwards is the tree from before. */
function refuseUnlessReleasable() {
  const dirty = git('status', '--porcelain')
  if (dirty) {
    throw new Error(
      `the working tree has uncommitted changes — commit or stash them first:\n${dirty}`
    )
  }

  const branch = git('branch', '--show-current')
  if (branch !== BRANCH) {
    const where = branch ? `on branch ${branch}` : 'on a detached HEAD'
    throw new Error(`${where}, and a release is cut from ${BRANCH} — switch first`)
  }

  /* Fetched rather than trusted: origin/main is only as fresh as the last
     fetch, and the comparison below exists precisely to refuse to tag a commit
     GitHub has never seen. The workflow checks the tag out on a runner, so a
     tag on a local-only commit is a build of something that is not there.
     The destination is spelled out rather than left to git's opportunistic
     update of the remote-tracking ref, which happens only while the remote's
     configured refspec covers the branch — and that is a setting this script
     does not own. Forced, like the default refspec, so an upstream that was
     rewritten is reported by the comparison below instead of by a fetch. */
  gitLive('fetch', REMOTE, `+${BRANCH}:refs/remotes/${REMOTE}/${BRANCH}`)

  const tracking = `${REMOTE}/${BRANCH}`
  const [ahead, behind] = git('rev-list', '--left-right', '--count', `HEAD...${tracking}`)
    .split(/\s+/)
    .map(Number)
  if (ahead && behind) {
    throw new Error(
      `${BRANCH} and ${tracking} have diverged: ${commits(ahead)} here that are not there, ` +
        `${commits(behind)} there that are not here — reconcile them first`
    )
  }
  if (ahead) throw new Error(`${BRANCH} is ${commits(ahead)} ahead of ${tracking} — push first`)
  if (behind) throw new Error(`${BRANCH} is ${commits(behind)} behind ${tracking} — pull first`)
}

/* Asked at the very front — before the version is raised, before the gates —
   and the placement is the whole value of the check rather than an accident of
   where it was written. The tag is created last of all because it is the point
   of no return, and a name already taken is the one way that last step could
   still fail: after main has been pushed, which is the one thing this script
   cannot take back. Hoisting the question to a moment when the answer costs
   nothing is what stops that. Do not move it down beside the `git tag` it is
   about. Local only: a tag on origin that is not here would have to have been
   pushed by somebody else, and `git push origin <tag>` refuses that one loudly
   on its own. */
function refuseIfTagged(tag) {
  if (git('tag', '--list', tag)) {
    throw new Error(`tag ${tag} already exists here — the release it names has been cut`)
  }
}

function runGate({ label, command, args }) {
  console.log(`\n→ ${label}`)
  const { status, error } = spawnSync(command, args, { cwd: root, stdio: 'inherit' })
  if (error) throw new Error(`${label} could not be started: ${error.message}`)
  if (status !== 0) throw new Error(`${label} failed`)
}

function main() {
  const step = parseStep(process.argv.slice(2))
  refuseUnlessReleasable()

  const before = readFileSync(confPath, 'utf8')
  const current = versionIn(before)
  const next = bumpVersion(current, step)
  const tag = `v${next}`
  refuseIfTagged(tag)

  console.log(`release: ${current} → ${next} (${step}), to be tagged ${tag}`)

  /* A listener that does nothing, and doing nothing is the whole of its job.
     The gates run with stdio inherited, so Ctrl-C goes to the foreground group
     — the child and node alike — and node's disposition for a signal nobody is
     listening for is to terminate on the spot, taking the restore below with
     it and leaving the version raised on disk. Registering any listener stops
     that: spawnSync comes back with `status: null, signal: 'SIGINT'`, which the
     `status !== 0` throw already covers, so an interrupted release ends down
     the same path as a red gate and puts the file back. It is registered here
     rather than at the top because up to this line there is nothing to undo. */
  process.on('SIGINT', () => {})

  /* Raised before the gates rather than after, so that what the three of them
     check is the tree that is about to be committed and tagged. Everything as
     far as the commit is inside the try, and the commit belongs there as much
     as the gates do: it fails for reasons that are nothing to do with a
     release — a hook, `commit.gpgsign`, an unset `user.email` on somebody
     else's machine — and a raise left on disk under `git commit … failed` says
     nothing about a version. Restoring after one is right because `--only`
     builds its commit through a temporary index: a rejected commit has created
     nothing, staged nothing, and nothing has been pushed at that point. */
  writeFileSync(confPath, withVersion(before, next))
  try {
    for (const gate of GATES) runGate(gate)

    /* One file, named on the commit rather than staged, because a gate can
       leave something else behind — cargo rewrites src-tauri/Cargo.lock when
       it feels like it — and a release commit quietly carrying that would be a
       second change nobody reviewed. src-tauri/tauri.conf.json is also the only
       file that should move: package.json and src-tauri/Cargo.toml keep their
       own numbers and mean nothing, which is smetana-j98's decision and the
       reason there is exactly one version here to raise. */
    console.log(`\n→ committing ${CONF}`)
    gitLive('commit', '--only', '-m', `release: ${tag}`, '--', CONF)
  } catch (e) {
    writeFileSync(confPath, before)
    throw new Error(`${e.message} — the release is off, and ${CONF} is back at ${current}`)
  }

  /* main first, the tag last. Pushing the tag alone would leave origin without
     the commit the raised version lives on, so main would trail its own
     release; and the tag is what starts the build, so up to that push
     everything is still undoable locally. */
  gitLive('push', REMOTE, BRANCH)
  gitLive('tag', tag)
  gitLive('push', REMOTE, tag)

  console.log(`\n✓ ${tag} pushed — the release workflow builds and publishes it from here`)
}

/* Run only when this file is the program. scripts/fetch-bd.mjs calls its main()
   at the top level and can: nothing imports it. This one is imported by
   tests/scripts/release.test.js for the pure halves above, and a module that
   pushes a tag when it is loaded has no business being in the tree. */
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main()
  } catch (e) {
    console.error(`release: ${e.message}`)
    process.exit(1)
  }
}
