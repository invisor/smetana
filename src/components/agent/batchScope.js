/* Where a tool call of an Autopilot batch ran, as the small tag at the right
   end of its row.

   The Rust side hands over the directory a Bash command opened with
   (`cd <absolute path> || exit 1;`, stripped out of the command text); this
   decides what that directory *is* to a person: the project's own root, the
   worktree of one of the board's tasks, a temp folder, the app bundle, or some
   other place. Pure — no Vue, no Tauri — so the whole table is tested.

   A worktree is matched to a task by the folder name, which is `<id>-<slug>`:
   the longest board id that is the folder's name or a prefix of it up to a
   `-` wins, so `smetana-th4` does not steal `smetana-th4n`'s folder. The tag
   then wears the colours `status.js` hashes for that id, which is what makes
   one task one hue wherever it appears. A worktree of a task that has left the
   board matches nothing and is drawn muted under its folder name, which is the
   honest reading rather than a colour for an id nobody can click. */
import { statusColors } from '../status/status.js'
import { basename } from '../../paths.js'

const MUTED = { fg: 'var(--text-muted)', bg: 'transparent', border: 'var(--border-subtle)' }
const ROOT = { fg: 'var(--text-muted)', bg: 'transparent', border: 'var(--border)' }
const TMP_ROOTS = ['/tmp', '/private/tmp', '/var/folders']

const trimEnd = (path) => (path.length > 1 ? path.replace(/\/+$/, '') : path)
const under = (path, root) => root !== '' && (path === root || path.startsWith(`${root}/`))

export function scopeOf(cwd, { project = '', issueIds = [], appDir = '' } = {}) {
  const root = trimEnd(project)
  if (cwd == null || cwd === '' || trimEnd(cwd) === root) {
    return { kind: 'root', label: 'root', title: root || (cwd ?? ''), task: null, colors: ROOT }
  }
  const path = trimEnd(cwd)
  const title = cwd

  const worktrees = `${root}/.worktrees/`
  if (root !== '' && path.startsWith(worktrees)) {
    const name = path.slice(worktrees.length).split('/')[0]
    let task = null
    for (const id of issueIds) {
      if ((id === name || name.startsWith(`${id}-`)) && (task === null || id.length > task.length)) task = id
    }
    if (task === null) return { kind: 'worktree', label: name, title, task: null, colors: MUTED }
    const { fg, bg, border } = statusColors(task)
    return { kind: 'worktree', label: task.slice(task.lastIndexOf('-') + 1), title, task, colors: { fg, bg, border } }
  }
  if (under(path, trimEnd(appDir))) return { kind: 'app', label: 'app', title, task: null, colors: MUTED }
  if (TMP_ROOTS.some((tmp) => under(path, tmp))) return { kind: 'tmp', label: '/tmp', title, task: null, colors: MUTED }
  return { kind: 'other', label: basename(path), title, task: null, colors: MUTED }
}
