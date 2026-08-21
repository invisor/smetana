/* What a name typed into the tree's draft row comes to, before anything is
   asked of the disk.

   A pure module for the reason the whole of that family is one: the field lives
   in a `.vue` file and no test in this repository can reach inside one, so the
   rule about what a person typed lives out here where a test can hold it.

   Three outcomes and not two, because "nothing" and "no" are different answers
   to the same keystroke. An empty field — or one holding only spaces, which
   looks exactly as empty — is somebody pressing Enter on a row they have
   changed their mind about: the draft goes away and nothing is said, the same
   as Esc. A name that cannot be used is a refusal with a sentence, because the
   person typed something and is owed the reason it did not take.

   The three refused shapes are the ones `reject_bad_name` refuses in Rust, and
   this is deliberately the same rule twice over rather than a check that
   replaces it: the field is one way in and the command is another, and the copy
   here is what keeps a hopeless name from costing a trip across the IPC at all.
   `a/b.js` in one keystroke is what VS Code does and is out of scope — a name
   here is one level of intent, so a separator is a refusal rather than
   something to split on. */

/* What the trimmed name is judged to be:
     'nothing'  — the field was empty; cancel, say nothing
     'refused'  — a name no entry can carry; a toast and nothing on disk
     'make'     — hand it to Rust
   The trimmed `name` travels with all three: a person who typed " notes.md "
   meant `notes.md`, and the shell of spaces around it is nobody's intent. */
export function checkNewName(raw = '') {
  const name = String(raw ?? '').trim()
  if (name === '') return { verdict: 'nothing', name }
  if (name === '.' || name === '..') return { verdict: 'refused', name }
  if (name.includes('/') || name.includes('\\')) return { verdict: 'refused', name }
  return { verdict: 'make', name }
}
