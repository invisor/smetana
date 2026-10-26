/* What the strip above the footer says about the connection to the server.

   The whole of the rule, pure, so that a `.vue` file — which no test here can
   reach — only draws what this answers. The input is the connection store's
   value in the shape the shell gives it (`src-tauri/src/split/state.rs`'s
   `View`), and the answer is `null` when there is nothing to say: the build
   without a server of its own, and a connection that is up and current.

   Two tones and no more. `wait` is a state that mends itself (connecting,
   reconnecting, a server older than this app that is waiting to be replaced);
   `blocked` is one that needs somebody (another protocol version, a device the
   server does not know). The words say what the app is doing with the data and
   with writes, because that is the part a person can act on: the last data
   stays on screen, marked stale, and a change is refused with this reason
   rather than kept and sent later. */

const KIND_WORDS = { run: 'Run', agent: 'Agent', session: 'Session', shell: 'Shell' }

/* One line for one piece of live work, as the server listed it. */
export function workLine(work) {
  const kind = KIND_WORDS[work?.kind] ?? 'Work'
  const label = typeof work?.label === 'string' && work.label !== '' ? work.label : null
  return label ? `${kind}: ${label}` : kind
}

/* The sentence about what a restart would end. */
export function workSentence(list) {
  const items = Array.isArray(list) ? list : []
  if (items.length === 0) return 'Nothing is running.'
  const lines = items.slice(0, 3).map(workLine)
  const more = items.length > 3 ? `, and ${items.length - 3} more` : ''
  const noun = items.length === 1 ? 'thing is' : 'things are'
  return `${items.length} ${noun} running and would end: ${lines.join('; ')}${more}.`
}

const STALE_NOTE = 'The last data stays on screen, marked stale. Changes are refused and not kept for later.'

function withReason(base, reason) {
  return typeof reason === 'string' && reason !== '' ? `${base} (${reason})` : base
}

export function connectionNotice(connection) {
  if (!connection || connection.mode !== 'split') return null
  switch (connection.state) {
    case 'connecting':
      return {
        tone: 'wait',
        icon: 'loader-circle',
        title: 'Connecting to the server',
        detail: withReason('Starting it if it is not running.', connection.reason),
        canRetry: true,
        restart: null
      }
    case 'reconnecting':
      return {
        tone: 'wait',
        icon: 'loader-circle',
        title: 'Connection to the server lost',
        detail: withReason(`Trying again. ${STALE_NOTE}`, connection.reason),
        canRetry: true,
        restart: null
      }
    case 'incompatible':
      return {
        tone: 'blocked',
        icon: 'triangle-alert',
        title: 'This app and the server are different versions',
        detail: withReason(STALE_NOTE, connection.reason),
        canRetry: true,
        restart: null
      }
    case 'unpaired':
      return {
        tone: 'blocked',
        icon: 'triangle-alert',
        title: 'This app is not paired with the server',
        detail: withReason(STALE_NOTE, connection.reason),
        canRetry: true,
        restart: null
      }
    case 'connected': {
      const outdated = connection.outdated
      if (!outdated) return null
      const versions = `The server is ${outdated.running}, this app ships ${outdated.bundled}.`
      if (connection.restartScheduled) {
        return {
          tone: 'wait',
          icon: 'refresh-cw',
          title: 'The server will restart when its work is done',
          detail: `${versions} Nothing is ended in the meantime.`,
          canRetry: false,
          restart: { canNow: false, canWhenIdle: false, canCancel: true }
        }
      }
      return {
        tone: 'wait',
        icon: 'refresh-cw',
        title: 'The server is older than this app',
        detail: outdated.canRestart
          ? `${versions} ${workSentence(outdated.work)} It keeps working as it is until you choose.`
          : `${versions} It keeps working as it is, but it cannot be restarted from here.`,
        canRetry: false,
        restart: outdated.canRestart ? { canNow: true, canWhenIdle: true, canCancel: false } : null
      }
    }
    default:
      return null
  }
}

/* Whether what is on screen is older than the server's truth. */
export function isStale(connection) {
  return !!connection && connection.mode === 'split' && connection.state !== 'connected'
}
