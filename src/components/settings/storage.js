/* What the Storage tab says, and nothing about how it looks.

   Another of the `branchChoice.js` family: the whole of a rule, pure, with no
   Vue and no DOM in it, living under the part of the interface it is a rule
   about. Here the rule is words rather than a choice — how many bytes read as a
   size, and which of five sentences belongs under a button that deletes
   somebody's pictures. It is out here for the same reason as the rest of the
   family: a `.vue` file is the one thing no test in this repository can reach,
   and the sentence a person reads before an irreversible press is worth pinning.

   The numbers come from `attachments_survey` whole, in the back end's own
   shape, the way `runs.js` keeps its `config` — a field this front end has not
   heard of must not silently read as one it has. */
import { basename } from '../../paths.js'

/* Binary units, spelled the way they are meant. The store's own ceiling is
   documented as 8 MiB in `attachments.rs`, so calling 1024 bytes a kilobyte
   here would put two vocabularies in one app for the sake of a familiar
   abbreviation. */
const UNITS = ['bytes', 'KiB', 'MiB', 'GiB', 'TiB']
const STEP = 1024

/* One decimal above a kilobyte and none below it: a screenshot is 3.4 MiB and
   nobody wants 3.417, while "1.0 bytes" is not a size at all. */
export function formatBytes(bytes) {
  /* `Number(null)` is 0, so the absence of a number has to be turned away
     before the conversion rather than after it: "0 bytes" under a heading that
     has not been answered yet reads as "your pictures are gone". */
  const size = typeof bytes === 'number' ? bytes : NaN
  if (!Number.isFinite(size) || size < 0) return '—'
  let value = size
  let unit = 0
  while (value >= STEP && unit < UNITS.length - 1) {
    value /= STEP
    unit += 1
  }
  if (unit === 0) return `${Math.round(value)} ${UNITS[0]}`
  return `${value.toFixed(1)} ${UNITS[unit]}`
}

/* "1 file" rather than "1 files", and the count is always spelled out: this
   number is half of what a person is deciding on. */
export function countFiles(files) {
  return files === 1 ? '1 file' : `${files} files`
}

/* A tally that is missing, or arrived in a shape this build does not know, is
   not zero — zero is a fact about the disk. */
const tally = (value) =>
  value && Number.isFinite(value.files) && Number.isFinite(value.bytes) ? value : null

/* How big the whole store is — every project's images and whatever is left in
   its root, which is more than the button can reach. That is deliberate: a
   person asking how much this has grown is asking about the directory, not
   about their share of it today. */
export function storageLine(survey) {
  const store = tally(survey?.store)
  if (!store) return 'Counting…'
  if (store.files === 0) return 'Nothing stored'
  return `${formatBytes(store.bytes)} in ${countFiles(store.files)}`
}

/* What the button would take, before it is pressed. Five outcomes, and the one
   thing they have in common is that none of them is silent about why there is
   nothing to do. */
export function removalLine(survey) {
  if (!survey) return 'Counting what is stored…'
  if (!survey.project) {
    return 'No project is open, so there is no board to say which images are still in use.'
  }
  const removable = tally(survey.removable)
  const kept = tally(survey.kept)
  if (!removable) return 'The storage could not be read.'
  if (removable.files === 0) {
    return kept && kept.files > 0
      ? 'Every image in this project belongs to a task that is still open.'
      : 'This project has no stored images.'
  }
  return `${countFiles(removable.files)} (${formatBytes(removable.bytes)}) in this project belong to no open task. Deleting them cannot be undone.`
}

/* Which project the button reaches, said plainly, because the answer to "what
   about my other project" is that the button physically cannot get there. */
export function scopeLine(survey) {
  if (!survey?.project) return ''
  return `Only images stored for ${basename(survey.project)} are touched; other projects are left alone.`
}

/* Whether there is anything for a press to do. A button that is live with
   nothing behind it teaches a person that the number above it means nothing. */
export function canClear(survey) {
  const removable = tally(survey?.removable)
  return Boolean(survey?.project) && Boolean(removable) && removable.files > 0
}

/* What the last press actually did — not what it was going to do. The two can
   differ: the rule is applied again inside the command, so an agent that filed
   a task in the second between the two saves its own screenshot. */
export function removedLine(cleaned) {
  if (!cleaned) return ''
  const removed = tally(cleaned.removed)
  const failed = Number(cleaned.failed) || 0
  const done =
    removed && removed.files > 0
      ? `Deleted ${countFiles(removed.files)} (${formatBytes(removed.bytes)}).`
      : 'Nothing was deleted.'
  return failed > 0 ? `${done} ${countFiles(failed)} could not be deleted.` : done
}
