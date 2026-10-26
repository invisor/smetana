/* The rules of the server-side folder browser, pure: what an address and a folder name may be, how a
   path is shown as steps, what a listing says about itself.

   Another of the `branchChoice.js` family - no Vue, no DOM, no Tauri - because the dialog that draws
   this is a `.vue` file and nothing here may hide in one. The server decides everything that
   matters (`smetana_core::placement` refuses a path outside its root and an address git must not be
   given); the checks in this file exist so that a person is told so before the round trip, and they
   are **never more permissive than Rust's**: `validCloneUrl` and `validFolderName` are the same
   rules, and the tests beside this file hold the examples both sides refuse.

   Design: `docs/superpowers/specs/2026-10-25-headless-server-design.md`, "Things that name a path on
   the client". */

/* A name that is one plain path segment: no separator, no `..`, no leading dot or dash, no control
   character. `valid_folder_name` in `placement.rs`. */
export function validFolderName(name) {
  return (
    typeof name === 'string' &&
    name.length > 0 &&
    name.length <= 255 &&
    !/^[.-]/.test(name) &&
    !/[/\\\0]/.test(name) &&
    // eslint-disable-next-line no-control-regex
    !/[\u0000-\u001f\u007f-\u009f]/.test(name)
  )
}

/* Whether an address is one `git clone` is told to try: `https://`, `http://`, `ssh://`, `git://` or
   the `user@host:path` form. Not an option (`-…`), not a remote-helper transport (`ext::…`), not a
   path on the server's own disk. `valid_clone_url` in `placement.rs`. */
export function validCloneUrl(url) {
  if (typeof url !== 'string') return false
  const text = url.trim()
  // eslint-disable-next-line no-control-regex
  if (!text || text.startsWith('-') || /[\u0000-\u001f\u007f-\u009f]/.test(text) || /\s/.test(text)) return false
  const lower = text.toLowerCase()
  if (['https://', 'http://', 'ssh://', 'git://'].some((scheme) => lower.startsWith(scheme))) return true
  if (lower.includes('::') || lower.includes('://')) return false
  const at = text.indexOf(':')
  if (at < 0) return false
  const host = text.slice(0, at)
  const path = text.slice(at + 1)
  return (host.includes('@') || host.includes('.')) && !/[/\\]/.test(host) && path.length > 0 && !path.startsWith(':')
}

/* The folder name an address ends in: `https://host/team/app.git` is `app`. `clone_name` in Rust. */
export function cloneName(url) {
  const trimmed = String(url ?? '').trim().replace(/\/+$/, '')
  const last = trimmed.split(/[/:]/).pop() ?? ''
  const name = last.endsWith('.git') ? last.slice(0, -4) : last
  return validFolderName(name) ? name : null
}

/* What is wrong with the clone form, in words for the field, or null. `name` is the optional
   override; empty means "take it from the address". */
export function cloneProblem(url, name) {
  if (!String(url ?? '').trim()) return null
  if (!validCloneUrl(url)) {
    return 'Use an https://, ssh:// or git:// address, or the user@host:path form.'
  }
  if (name && !validFolderName(name)) return 'The folder name cannot contain slashes or start with a dot.'
  if (!name && !cloneName(url)) return 'Could not work out a folder name from the address; type one.'
  return null
}

/* The steps from the root down to `path`, for the bar above the list: the root is named "Home" and
   every step carries the full path to go back to it. A path that is not under the root gives just the
   root, which is the only place the browser can be in that case. */
export function steps(root, path) {
  if (!root) return []
  const top = [{ name: 'Home', path: root }]
  const base = root.replace(/\/+$/, '')
  if (!path || path === root || !path.startsWith(`${base}/`)) return top
  const rest = path.slice(base.length).split('/').filter(Boolean)
  let at = base
  return [
    ...top,
    ...rest.map((name) => {
      at = `${at}/${name}`
      return { name, path: at }
    })
  ]
}

/* What a listing says about its folder, as one line under the bar: nothing to say for an ordinary
   folder, and a short note for a repository, a tracked project, or a list that was cut. */
export function listingNote(listing) {
  if (!listing) return ''
  const notes = []
  if (listing.repo) notes.push(listing.tracker ? 'A Git repository with a tracker' : 'A Git repository')
  if (listing.truncated) notes.push('Only the first part of a long list is shown')
  return notes.join(' - ')
}

/* The empty state of a folder with no folders in it. */
export const EMPTY_FOLDER = 'No folders in here'
