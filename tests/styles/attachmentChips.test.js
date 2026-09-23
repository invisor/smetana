import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const read = (name) => readFileSync(resolve(process.cwd(), name), 'utf8')
const stylesheet = read('src/styles/sm-prose.css')
const userMessage = read('src/components/conversation/UserMessage.vue')

/* Components are not mounted by this repository's test runner, so this is a
   source-level regression check for the two parts that make the failure
   possible: UserMessage must emit every attachment as a sibling flex item, and
   the attachment rule must defeat the prose-list rhythm that otherwise gives
   every sibling after the first a top margin. */
describe('sent attachment chips', () => {
  it('keeps multiple chips in one flex row on the same vertical line', () => {
    expect(userMessage).toContain('<li v-for="path in attachments" :key="path">')
    expect(stylesheet).toMatch(/\.sm-prose ul\[data-attachments\]\s*\{[\s\S]*?display:flex;[\s\S]*?flex-wrap:wrap;/)

    const attachmentItems = stylesheet.match(
      /\.sm-prose ul\[data-attachments\] > li\s*\{([\s\S]*?)\n\}/
    )
    expect(attachmentItems, 'attachment items keep their own cascade boundary').not.toBeNull()
    expect(attachmentItems[1]).toMatch(/margin-top:var\(--space-0\);/)

    expect(stylesheet).toMatch(/\.sm-prose li \+ li\{ margin-top:var\(--prose-tight-gap\) \}/)
  })
})
