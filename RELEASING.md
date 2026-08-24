# Releasing Smetana

How a release is cut, what the workflow does with it, and where the signing key came from. None of
this is needed to use the app or to work on it — see [`README.md`](README.md) for that.

## One command, from `main`

```sh
npm run release            # 0.1.0 → 0.1.1
npm run release -- minor   # 0.1.0 → 0.2.0
npm run release -- major   # 0.1.0 → 1.0.0
```

The step is `patch` when no argument is given, which is what most releases are. An exact version
number is not accepted as an argument at all, and that absence is the point rather than a gap: the
number is computed from `version` in `src-tauri/tauri.conf.json`, so the only way to get it wrong —
naming it yourself — is not available.

`scripts/release.mjs` raises that one field, runs all three gates (`npm test`, `npm run build`,
`cargo test --manifest-path src-tauri/Cargo.toml`), then commits, pushes `main`, tags `v<x.y.z>` and
pushes the tag, in that order. That is the same sequence a person used to run by hand, and it is
still exactly what happens — what the script adds is that it cannot misremember the number and will
not tag a commit no gate has seen. The version is raised *before* the gates so that what the three of
them check is the tree that is about to be committed and tagged; a red gate, an interrupted run or a
refused commit stops the release before the tag exists and puts `tauri.conf.json` back at the version
it started from.

`main` first and the tag last: pushing the tag alone would leave origin without the commit the raised
version lives on, and the tag is what starts the build, so up to that push everything is still
undoable locally. Nothing after the pushed tag is the script's business — it never touches the GitHub
API, because the build and the signature belong to a runner that holds the secrets and no laptop
does.

## What it refuses, and when

There are **four** refusals, all of them before anything on disk has been touched, and each one names
its own reason rather than saying "cannot release":

1. **A dirty working tree** — the uncommitted paths are printed with the refusal.
2. **Any branch but `main`**, a detached HEAD included.
3. **`main` and `origin/main` out of step.** `origin/main` is fetched rather than trusted first, and
   the three cases are told apart in the message: diverged (commits on each side), ahead (push
   first), behind (pull first). Tagging a commit GitHub has never seen would be a build of something
   that is not there, since the workflow checks the tag out on a runner.
4. **A tag of the name it is about to create already exists here.** Asked at the very front, before
   the version is raised and before the gates, because the tag is created last of all and is the
   point of no return: a name already taken is the one way that last step could still fail, after
   `main` has been pushed. Local only — a tag on origin that is not here would have to have been
   pushed by somebody else, and `git push origin <tag>` refuses that one loudly on its own.

The count is worth stating because the number was written down as three when the script's own
`refuseUnlessReleasable` and `refuseIfTagged` add up to four, of which the third has three distinct
messages. `tests/scripts/release.test.js` covers the pure halves of the script; the refusals
themselves are not covered by any test.

## What the tag sets off

Pushing that tag is what the rest hangs on. `.github/workflows/release.yml` listens on
`tags: - 'v*'` and on nothing else.

`version` in `src-tauri/tauri.conf.json` is the single source of the app's version: it is what the
app reports about itself and the number to quote in a bug report, while `package.json` and
`src-tauri/Cargo.toml` carry their own and mean nothing. The workflow's `check` job strips the same
`v` prefix (`${GITHUB_REF_NAME#v}`) and compares the two, failing and naming both when they disagree
— otherwise `latest.json` would announce a version the bundle does not carry, and every installed
copy would try to update to it for ever. That job also refuses to build while
`plugins.updater.pubkey` is empty, rather than publish a release nobody can install as an update.

The two sides fail unequally, and the script's side is the quiet one: change the prefix there and the
push matches no trigger, the script prints its success line and nothing builds. `npm test` cannot see
the workflow and `cargo test` cannot see the script.

The `release` job then builds a macOS arm64 bundle — one matrix row, because
`scripts/fetch-bd.mjs` fetches the sidecar for the host triple and `bundle.externalBin` looks it up
by the build target's — and publishes a GitHub release holding the `.dmg`, the `.app.tar.gz` an
updater installs, its `.sig`, and `latest.json`. That last is what `plugins.updater.endpoints` points
at — `https://github.com/invisor/smetana/releases/latest/download/latest.json`, reachable with no
token because this repository is public, and the reason `releaseDraft` and `prerelease` are both
false: that URL resolves against the latest **published** release only.

What the app does with that feed — when it checks, what it downloads by itself, what it refuses to do
while a run is live — is not release mechanics and is not here. `.claude/rules/updates.md` is where
it is written down.

## The signing key

Three things had to be done by a person before the first release could work, and all three are in
place, done once by the repository owner. They are written out here as what happened rather than as
instructions, because each of them carries a detail that is expensive to rediscover and there is
nowhere else it is written down.

1. `npm run tauri signer generate` produced the minisign key pair, and it was given a password. The
   command prints both halves to the terminal — two base64 blobs, under `Private:` and `Public:` —
   and writes nothing to disk unless you pass `-w <path>`. Tauri's update signature is mandatory and
   cannot be turned off.
2. The public half is committed, as `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`. The
   `check` job in the workflow refuses to build while that field is empty, rather than publish a
   release nobody can install as an update. It now guards against somebody emptying it rather than
   against the first release going out unsigned.
3. `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are set as repository
   secrets, under Settings → Secrets and variables → Actions. The first holds the blob printed under
   `Private:`; a pair written out with `-w` goes in as that file's **contents** and never as a path
   to it — the CLI accepts a path when it runs on your own machine, which is the tempting thing to
   paste, and on a runner a path resolves to nothing. The second holds the password from step 1, and
   forgetting it fails the build with `incorrect updater private key password`, which reads like a
   wrong password rather than a missing one and costs somebody the debugging twice.

The private half and that password live in the repository owner's password manager. Neither has ever
been in this tree, and neither belongs in a run log.

**Losing the private key means every already-installed copy can never be updated again.** There is no
way back from it: a copy already out there accepts only an update signed by the key whose public half
it was built with, so generating a fresh pair is not a repair — it abandons everybody already running
the app. Keep it somewhere that outlives the machine it was generated on.

## Building a bundle locally

One local consequence, now that `pubkey` is no longer empty: `npm run tauri build` refuses to bundle
with "A public key has been found, but no private key", the bundler looking for something to sign the
updater archive with and finding nothing in the environment. That is the intended state rather than a
fault: the private key lives in the repository secrets, and a build that signs an updater archive is
the workflow's job and not a laptop's. `npm run tauri build -- --no-sign` remains the way past it —
it skips the updater signature along with the code signing — for that reason now rather than because
the key was missing from the conf.

`npm run tauri dev` is unaffected, and so is everything `npm test`, `npm run build` and `cargo test`
do — none of them bundles.
