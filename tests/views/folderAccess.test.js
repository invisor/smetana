import { describe, expect, it } from 'vitest'
import { folderRefusedHasReset, folderRefusedNotice } from '../../src/views/folderAccess.js'

const FORMS = ['reset', 'full-disk-access', 'unavailable']

/* The state this notice replaced said "bd is failing — most often the tracker
   was made by an older bd than this build ships", under a button that runs a
   database migration, about a folder macOS was refusing to open. Both halves of
   that were false, and the first two tests are those two falsehoods written as
   rules: the sentence has to name the permission, and it must not put the
   tracker or its data in the frame. */
describe('the notice for a refused folder', () => {
  it('names the permission and clears the tracker of it, in every form', () => {
    for (const form of FORMS) {
      const { title, description } = folderRefusedNotice(form)

      expect(title.toLowerCase()).toContain('permission')
      expect(description).toContain('refusing this app access')
      expect(description).toContain('not the tracker or the data in it')
    }
  })

  it('says nothing about bd, its version or a repair of the data', () => {
    for (const form of FORMS) {
      const said = Object.values(folderRefusedNotice(form)).join(' ').toLowerCase()

      expect(said).not.toContain('bd')
      expect(said).not.toContain('migrat')
      expect(said).not.toContain('.beads')
    }
  })

  /* There is no confirmation dialog in front of the button, so the one thing
     that cannot be taken back — the app going away and coming up again — has to
     be said before it happens. In this app's own word for that act, which is
     "restart" (`components/settings/update.js`), not a second verb for one
     thing. */
  it('warns that the app restarts, and only where the button is', () => {
    expect(folderRefusedNotice('reset').description).toContain('restarts the app')
    expect(folderRefusedNotice('full-disk-access').description).not.toContain('restart')
    expect(folderRefusedNotice('unavailable').description).not.toContain('restart')
    for (const form of FORMS) {
      expect(folderRefusedNotice(form).description).not.toContain('relaunch')
    }
  })

  /* The whole of the Full Disk Access case: macOS never prompts for it, so the
     one thing this form must not do is promise a dialog, and the one thing it
     must do is name where the switch actually is. */
  it('sends a person to System Settings rather than promising a prompt', () => {
    const { description } = folderRefusedNotice('full-disk-access')

    expect(description).toContain('Full Disk Access')
    /* The route as this tree writes one, with the arrow `README.md` and the
       release workflow use, and the app under the name macOS shows in that
       list — `productName` is lower case. */
    expect(description).toContain('System Settings → Privacy & Security')
    expect(description).toContain('grant smetana Full Disk Access')
    expect(description).not.toContain('ask again')
  })

  /* `tccutil` is macOS's. Elsewhere the same refusal is reported with no button
     under it, so the sentence has to carry the whole of what to do — and it must
     not name a system this build cannot know it is running on. */
  it('holds up without a button, and does not claim to be on macOS', () => {
    const { description } = folderRefusedNotice('unavailable')

    expect(description).not.toContain('macOS')
    expect(description).toContain('Grant this app access')
  })

  it('names macOS in both forms that are macOS\'s own', () => {
    expect(folderRefusedNotice('reset').description).toContain('macOS')
    expect(folderRefusedNotice('full-disk-access').description).toContain('macOS')
  })

  /* An answer from a build this one does not know about, or none at all: the
     form that offers nothing and claims nothing about the platform. */
  it('an unrecognized answer falls to the form that promises least', () => {
    const fallback = folderRefusedNotice('unavailable')

    expect(folderRefusedNotice('something-new')).toEqual(fallback)
    expect(folderRefusedNotice(undefined)).toEqual(fallback)
  })

  /* The glyph is the same in all three, and it has to be one `core/icons.js`
     registers: an unregistered name draws nothing at all. */
  it('draws a registered glyph', () => {
    for (const form of FORMS) expect(folderRefusedNotice(form).icon).toBe('lock')
  })
})

describe('whether a button goes under it', () => {
  /* One of the three, and exactly one. Pressing `tccutil reset` for the Full
     Disk Access case would clear a grant macOS will never prompt for again —
     the person would have to find System Settings anyway, having lost the
     access they had. */
  it('is offered for a promptable folder and for nothing else', () => {
    expect(folderRefusedHasReset('reset')).toBe(true)
    expect(folderRefusedHasReset('full-disk-access')).toBe(false)
    expect(folderRefusedHasReset('unavailable')).toBe(false)
    expect(folderRefusedHasReset(undefined)).toBe(false)
  })
})
