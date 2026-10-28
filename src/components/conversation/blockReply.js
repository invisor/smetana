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

/* After the interrupt: whether the turn has closed and the approval may go.
   Anything but a busy state counts, `failed` and `exited` included: sending is
   then the call that reports why it cannot, which is better than waiting for a
   state that will not come. */
export function turnClosed(state) {
  return !isBusy(state)
}
