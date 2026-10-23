import { describe, expect, it } from 'vitest'
import { approvalText, denialReason, refusalText } from '../../../src/components/conversation/autoModeBlock.js'

describe('the auto-mode block replies', () => {
  it('names the tool and the command, then quotes the block', () => {
    expect(approvalText('Bash', 'git push --force', 'denied by auto mode: force push')).toBe(
      'I allow this action: Bash git push --force. Do exactly that and nothing beyond it.\n\nThe block said: denied by auto mode: force push'
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
})
