/* The Server tab's rules, pure: which server this device uses and how that is said, whether a pasted
   code can be right, and what a switch to a remote server has to ask first.

   Out of the component for the reason every module beside this one is: a `.vue` file is the one thing
   no test in this repository can reach. Design:
   `docs/superpowers/specs/2026-10-25-headless-server-design.md`, "Local placement" ("Switching the
   desktop to a remote server") and "Remote placement".

   **What switching does and does not do.** The server on this machine is left running and is not
   started again while a remote one is chosen. If it has work in it the person is told so before
   anything changes, and chooses between leaving it running and having it stop once that work has
   finished (it ends nothing; `server_restart` with `stop-when-idle`). */
import { workSentence } from '../shell/connectionNotice.js'

const PHONE_PREFIX = 'smetana1:'
const DESKTOP_PREFIX = 'smetana2:'

/* What is wrong with the code in the field, in a line for under it, or null: nothing typed yet, or
   something that can be tried. `smetanad pair --kind desktop` prints the code; a phone's code has a
   different prefix and would be refused after a wait, so it is refused here, at once. */
export function ticketProblem(text) {
  const value = String(text ?? '').trim()
  if (value === '') return null
  if (value.startsWith(PHONE_PREFIX)) {
    return 'That is a code for a phone. On the server run smetanad pair --kind desktop for this app.'
  }
  if (!value.startsWith(DESKTOP_PREFIX)) return 'That is not a Smetana pairing code. It starts with smetana2:.'
  if (/\s/.test(value)) return 'The code has a space or a line break in it. Paste it as one line.'
  if (value.length < DESKTOP_PREFIX.length + 20) return 'The code looks cut short. Copy all of it.'
  return null
}

/* The key of a server as a person can recognise it without reading all of it. */
export function shortNode(node) {
  const text = String(node ?? '')
  return text.length > 16 ? `${text.slice(0, 8)}…${text.slice(-6)}` : text
}

/* The host of a relay address, for "through relay.example.com". */
export function relayHost(relay) {
  try {
    return new URL(relay).host
  } catch {
    return String(relay ?? '')
  }
}

/* Which server this device uses, in a sentence. `target` is the shell's: `{ kind: 'local' }` or
   `{ kind: 'remote', node, relay }`. */
export function targetLine(target) {
  if (target?.kind === 'remote') {
    return `A server on another machine, ${shortNode(target.node)}, reached through ${relayHost(target.relay)}.`
  }
  return 'The server on this machine, which this app starts and keeps running.'
}

/* How the connection to it is going, in a short phrase for beside the sentence above. `connection`
   is the store's value. */
export function stateLine(connection) {
  switch (connection?.state) {
    case 'connected':
      return connection.server?.version ? `Connected, server ${connection.server.version}` : 'Connected'
    case 'connecting':
      return 'Connecting'
    case 'reconnecting':
      return 'Connection lost, trying again'
    case 'incompatible':
      return 'Different versions of the protocol'
    case 'unpaired':
      return 'Not paired with this server'
    default:
      return ''
  }
}

/* What pressing "Use this server" does first. `work` is what the local server has running, or null
   when it has not been asked yet (and `askedLocal` says whether it needs to be: only a device that
   is using the local server has one to leave behind).

   - `problem`: the code cannot be right; nothing is attempted.
   - `ask`: there is live work on the server being left; the person chooses what happens to it.
   - `switch`: go. The local server is left running unless `stopLocal`. */
export function nextStep({ ticket, usingLocal, work }) {
  const problem = ticketProblem(ticket)
  if (problem) return { kind: 'problem', problem }
  if (String(ticket ?? '').trim() === '') return { kind: 'problem', problem: 'Paste the code from the server first.' }
  if (usingLocal && Array.isArray(work) && work.length > 0) return { kind: 'ask', work }
  return { kind: 'switch', stopLocal: false }
}

/* The words over the choice when the local server has work in it. */
export function askSentence(work) {
  return `The server on this machine has work in it. ${workSentence(work)} Idle agents are ended when it stops and offered back afterwards; everything else finishes first.`
}

export const LEAVE_RUNNING = 'Leave it running'
export const STOP_WHEN_FINISHED = 'Stop it when it is finished'

/* What the shell reported about the local server after the switch (`localStop` in the answer of
   `server_use_remote`), as a line, or '' when there is nothing to add. */
export function localStopLine(answer) {
  const stop = answer?.localStop
  if (stop === 'scheduled') return 'The server on this machine will stop once its work is finished.'
  if (stop === 'stopping') return 'The server on this machine is stopping.'
  if (stop && typeof stop === 'object' && stop.failed) {
    return 'The server on this machine could not be asked to stop. It is still running.'
  }
  return ''
}

/* The line under the pairing button while it waits: confirming happens at the server, in the
   terminal that ran `smetanad pair`, and that is where a person who is stuck should look. */
export const WAITING_FOR_CONFIRMATION =
  'Waiting for you to confirm at the server, in the terminal where you ran smetanad pair…'
