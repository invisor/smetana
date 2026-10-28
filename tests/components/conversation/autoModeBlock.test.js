import { describe, expect, it } from 'vitest'
import { approvalText, blockButtonsShown, denialReason, refusalText } from '../../../src/components/conversation/autoModeBlock.js'

describe('the auto-mode block replies', () => {
  const DENIAL =
    'Permission for this action was denied by the Claude Code auto mode classifier. Reason: [Production Reads].'

  it('quotes the whole command and the category, and states the approval outright', () => {
    const command = `ssh -o BatchMode=yes prod-db.invalid '${'docker compose ps; '.repeat(20)}'`
    expect(approvalText('Bash', `${command.slice(0, 137)}...`, DENIAL, command)).toBe(
      'I explicitly approve this one action, which the auto mode classifier refused as [Production Reads]:\n\n' +
        `Bash: ${command}\n\n` +
        'I have read the reason and I approve exactly this command, against exactly the target it names, once. ' +
        'Do exactly that and nothing beyond it.'
    )
  })

  it('falls back to the clipped detail when the journal has no blocked input', () => {
    for (const none of [undefined, null, '']) {
      expect(approvalText('Bash', 'git push --force', DENIAL, none)).toContain('\n\nBash: git push --force\n\n')
    }
  })

  it('names the text itself as the category when the denial has no bracket', () => {
    expect(approvalText('Write', '/etc/hosts', 'denied by auto mode', '/etc/hosts')).toContain(
      'refused as [denied by auto mode]:'
    )
  })

  it('refuses by naming the same action', () => {
    expect(refusalText('Write', '/etc/hosts')).toBe(
      'Do not do this: Write /etc/hosts. Find a way without it, or stop and tell me what you need.'
    )
  })

  it('shows the bracketed category as the reason, or the text when there is none', () => {
    const denial =
      'Permission for this action was denied by the Claude Code auto mode classifier. Reason: [Modify Shared Resources].'
    expect(denialReason(denial)).toBe('Modify Shared Resources')
    expect(denialReason('some other text')).toBe('some other text')
    expect(denialReason(null)).toBe('')
  })

  it('offers the buttons only while the block is open, a turn may be sent and none is in flight', () => {
    const open = { blockOpen: true, composerShown: true, replying: false }
    expect(blockButtonsShown(open)).toBe(true)
    expect(blockButtonsShown({ ...open, blockOpen: false })).toBe(false)
    expect(blockButtonsShown({ ...open, composerShown: false })).toBe(false)
    expect(blockButtonsShown({ ...open, replying: true })).toBe(false)
  })
})
