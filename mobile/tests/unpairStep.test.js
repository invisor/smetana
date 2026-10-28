import { describe, expect, it } from 'vitest'
import { ASKED, IDLE, confirms, nextUnpairStep } from '../src/unpairStep.js'

describe('the two taps that unpair', () => {
  it('the first tap only asks', () => {
    expect(nextUnpairStep(IDLE, 'unpair')).toBe(ASKED)
    expect(confirms(IDLE, 'unpair')).toBe(false)
  })

  it('confirms only after it has asked', () => {
    expect(confirms(ASKED, 'confirm')).toBe(true)
    expect(confirms(IDLE, 'confirm')).toBe(false)
    expect(nextUnpairStep(IDLE, 'confirm')).toBe(IDLE)
  })

  it('goes back to idle on cancel and after a confirm', () => {
    expect(nextUnpairStep(ASKED, 'cancel')).toBe(IDLE)
    expect(nextUnpairStep(ASKED, 'confirm')).toBe(IDLE)
  })

  it('ignores a press that means nothing in the step it finds', () => {
    expect(nextUnpairStep(IDLE, 'cancel')).toBe(IDLE)
    expect(nextUnpairStep(ASKED, 'unpair')).toBe(ASKED)
    expect(nextUnpairStep(ASKED, 'nonsense')).toBe(ASKED)
  })
})
