import { describe, expect, it } from 'vitest'
import {
  CAPTION_LEAD,
  CONFIG_FILE,
  SETUP_LABEL,
  unlistedBlock
} from '../../../src/components/git/unlistedRepos.js'

describe('what the Git panel says about a repository the config does not name', () => {
  /* The whole of the first case: a panel that is quiet by design stays quiet
     when it has nothing to say. Every project set up properly is this one. */
  it('says nothing at all when there is nothing to say', () => {
    expect(unlistedBlock([])).toBe(null)
    expect(unlistedBlock(null)).toBe(null)
    expect(unlistedBlock(undefined)).toBe(null)
  })

  it('names the one folder, and says so in the singular', () => {
    const block = unlistedBlock(['newrepo'])
    expect(block.names).toEqual(['newrepo'])
    expect(block.summary).toBe(`1 repository is in this project but not in ${CONFIG_FILE}`)
  })

  it('names every folder, and says so in the plural', () => {
    const block = unlistedBlock(['newrepo', 'vendor-fork'])
    expect(block.names).toEqual(['newrepo', 'vendor-fork'])
    expect(block.summary).toBe(`2 repositories are in this project but not in ${CONFIG_FILE}`)
  })

  /* The caption is the same two pieces however many names follow it — the
     count is on the rows, and a caption counting them would say twice what a
     person can see once. The file is its own piece because it is an
     identifier: whatever draws it draws it in mono. */
  it('captions both the same way, with the file as a piece of its own', () => {
    for (const names of [['newrepo'], ['newrepo', 'vendor-fork', 'notes']]) {
      const block = unlistedBlock(names)
      expect(block.lead).toBe(CAPTION_LEAD)
      expect(block.file).toBe(CONFIG_FILE)
    }
  })

  /* `repos.rs` answers in the listing's own order, sorted there so that two
     machines looking at one folder draw one list. Re-sorting it here would be
     this file having an opinion about an order that already means something. */
  it('keeps the order it was given', () => {
    expect(unlistedBlock(['zephyr', 'admin']).names).toEqual(['zephyr', 'admin'])
  })

  /* Neither can come out of the backend today. Both would draw a row that is
     about nothing, and a list is cheap to make honest where it is read. */
  it('drops a blank name and a repeated one rather than drawing a row for it', () => {
    const block = unlistedBlock(['newrepo', '  ', 'newrepo', '', 7, 'notes'])
    expect(block.names).toEqual(['newrepo', 'notes'])
    expect(block.summary).toBe(`2 repositories are in this project but not in ${CONFIG_FILE}`)
  })

  /* One act with two doors must not have two names: the project row's own
     right-click menu is where this verb already lives. */
  it('offers the way out in the words the project menu already uses', () => {
    expect(SETUP_LABEL).toBe('Set up again')
  })
})
