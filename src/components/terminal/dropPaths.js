/* What a file dropped on the terminal panel becomes in the session's input.

   The gesture is the one iTerm and Terminal.app have: the absolute path is
   typed for the person, and nothing is sent. A path is almost always part of a
   sentence — "look at X and fix it" — rather than the whole of it, so the text
   ends in a single space and the person goes on typing around what landed.

   Pulled out of `TerminalView.vue` because a `.vue` file is the one thing no
   test in this repository can reach, and this is the whole of the rule: the
   component does the aiming and the sending, this decides the characters. */

/* The characters a path may carry and still go in bare — shlex.quote's own set,
   which is the conservative reading rather than a list of what happens to be
   dangerous today. Everything outside it takes quotes: whitespace, the quote
   characters themselves, `$`, a backtick, a backslash, the glob characters, and
   also `~`, which only ever leads a path a shell would expand.

   Quoting only when required is a decision about reading rather than about
   safety, and it goes the other way from "always quote": the ordinary case is a
   path with nothing special in it, and a bare one sits in the middle of a
   half-typed sentence far better than a quoted one. */
const BARE = /^[A-Za-z0-9_@%+=:,./-]+$/

/* One path, ready to stand next to a person's own words.

   Single quotes and not double: inside single quotes a shell expands nothing at
   all, so `$HOME` and a backtick in a file's name stay the characters they are.
   The one thing single quotes cannot hold is a single quote, and the classic
   escape is to leave the quoted run, write an escaped quote and open a new run
   — `'\''` — which is text no shell can misread. */
export function quotePath(path) {
  if (BARE.test(path)) return path
  return `'${path.split("'").join("'\\''")}'`
}

/* A path this gesture will not type at all: one carrying a control character.

   Quoting is a defence against a *shell parse*, and this string is not parsed
   by a shell — it is typed into a PTY, where the line discipline and whatever
   TUI is running read the bytes before any shell sees a word. Single quotes do
   nothing to them there. A line feed or a carriage return in a filename **is** a
   Return: in an agent it submits the person's half-written message on their
   behalf, which is the one thing this whole gesture is built not to do and the
   one thing that cannot be taken back; in a shell, bash takes the line with the
   quote still open, drops to PS2, and the next thing typed runs the dropped file
   as a command. A tab fires completion in a shell and moves focus in a TUI, and
   an escape character is read as the start of a sequence. POSIX allows every one
   of these bytes in a filename, so this is a real path rather than a hypothesis.

   Refused rather than repaired. Stripping the character or escaping it would
   produce a string that no longer names the file somebody dropped, and a path
   that quietly points at nothing is worse than no path at all: the person can
   see that nothing was inserted. */
const CONTROL = /[\u0000-\u001f\u007f]/

/* Every path a drop carried that this gesture can type, in the order the event
   gave them, joined by one space and followed by one more.

   The refusal is per path and not per drop: three files of which one is
   pathological still insert the other two, in the order they came. Nothing to
   say is the empty string rather than a lone space: a drop that carried no path
   this can type must not move the cursor of a session somebody is typing in. */
export function dropText(paths) {
  const list = (paths ?? []).filter(
    (path) => typeof path === 'string' && path !== '' && !CONTROL.test(path)
  )
  if (list.length === 0) return ''
  return `${list.map(quotePath).join(' ')} `
}
