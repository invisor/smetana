/* When an auto-mode block's Allow may be sent, as a pure module: the rule lives
   outside `ConversationView.vue` because that file is the one thing no test
   here can reach.

   Why Allow is not an ordinary send. Measured against Claude Code 2.1.310
   (smetana-io57): a stream-json `user` line written while a turn is running is
   not a user message to the harness. The transcript records it as
   `queue-operation enqueue` -> `attachment: queued_command` -> `remove`; the
   model reads it between two tool calls, but the classifier judges what the
   person approved from the person's messages, and the retried command was
   refused again, with the old wording and with the new. The same text sent
   once the turn had ended is recorded as a `user` message, and the retry ran.
   The card is drawn under a result the agent has already moved past, so the
   press nearly always lands mid-turn, and a busy agent has to be stopped first.

   Interrupt, wait, then send, rather than send and then interrupt. Both orders
   were measured and both ended in a `user` message and a retry that ran; the
   second one leaves a window, between the line being queued and the interrupt
   arriving, in which a finishing tool call drains the queue as a
   `queued_command` again. The first has no such window, and the wait it needs
   is the session's own `state` leaving `running`/`needs-you`, never a timer. */
import { isBusy } from './journal.js'

/* 'interrupt-then-send' while the agent is mid-turn, 'send' when it is idle. */
export function allowPlan(state) {
  return isBusy(state) ? 'interrupt-then-send' : 'send'
}

/* While waiting for the interrupted turn to close: 'wait' while it is still
   running, 'ready' once it has closed (`failed` and `exited` included: sending
   is then the call that reports why it cannot, better than waiting for a state
   that will not come), and 'abort' when a permission question has taken over
   (`needs-you`). That question is a card the person has to answer, it can keep
   the state there indefinitely, and the buttons must come back rather than
   stay hidden behind a wait that has no end. */
export function waitVerdict(state) {
  if (state === 'running') return 'wait'
  if (state === 'needs-you') return 'abort'
  return 'ready'
}

/* Whether the approval may go once the wait is over. It goes to the session
   that was pressed, captured when the press happened, and only while the panel
   still shows that session and its block is still open. The panel is one
   instance re-pointed at another agent when the person clicks one, so a
   decision read off the current props after a wait of seconds would send an
   approval of one agent's action to another. */
export function replyTarget({ capturedId, currentId, blockOpen }) {
  return capturedId !== null && capturedId !== undefined && capturedId === currentId && blockOpen ? 'send' : 'abort'
}
