---
paths:
  - "src/components/kanban/parked.js"
---

# Answering what a run could not

A task an automatic run could not settle is `parked` with the question written into its notes as a
`parked:` line — `runs::queue::parking_note` on the app's side, `running-tasks` when a lead does it
by hand. `ResolveTask` is the way back: an agent session that puts those questions to the person at
the terminal, writes what they answer into the issue, and unparks it.

The rules are `smetana:resolving-questions`, and three of them are in `prompt.rs` as well, for the
reason `STANDARD` is: an `Inline` harness may find no skill text at all. Ask one at a time and
**answer none of them yourself** — the task is parked precisely because guessing was not good enough
for the agent that stopped. The answers go into the **description**, because that is the spec
`provisioning` reads and a decision recorded only in the notes is one the implementer never sees, and
a `resolved:` line goes into the notes beside each `parked:` one. And the status is the **last**
write, the same rule filing keeps: a session interrupted halfway leaves the task parked rather than
back in the queue with the answer written nowhere.

The front end's half is `components/kanban/parked.js`, another of the `branchChoice.js` family, and
it holds the pairing rule the notes are read by: **everything below the last `resolved:` line is
still open.** Not a question matched to its own answer, which would need the two written in step —
a person settling three questions in one sentence writes one `resolved:`, and a positional pairing
would call two of them unanswered. The sequence is what is true instead, because a resolving session
answers everything open at that moment and only then unparks.

Three places act on it and they have to agree, which is why the rule is one pure file rather than
three conditions. A parked card's menu offers "Answer questions" first, above the play. The play
itself is dead there (`runnableTask` in `DesktopApp.vue`), and that is not tidiness: without it the
play is the way around the dialog, one row above it in the same menu. And moving a parked card to
Ready asks first, quoting the open questions verbatim — three ways out, `Move anyway` writing the
status exactly as the menu always did, with no note invented on the person's behalf. Only Ready
asks: Done decides the question no longer matters and Pinned takes the task off the queue, while
Ready is the one that hands it to an agent with the question still open.
