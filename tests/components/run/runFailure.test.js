import { describe, it, expect } from 'vitest'
import { runFailure } from '../../../src/components/run/runFailure.js'

describe('runFailure', () => {
  it('shows the server detail of a bad settings error', () => {
    expect(runFailure({ kind: 'bad_settings', detail: 'x' })).toBe('x')
  })
  it('says the project has no run configuration', () => {
    expect(runFailure({ kind: 'not_configured' })).toBe('This project has no run configuration yet.')
  })
  it('names the scope that is already running', () => {
    expect(runFailure({ kind: 'already_running', detail: { scope: 'task a-1' } })).toBe(
      'A run over task a-1 is already going in this project.'
    )
  })
  it('says the previous run is still finishing', () => {
    expect(runFailure({ kind: 'winding_down' })).toBe('The previous run is still finishing. Try again in a moment.')
  })
  it('passes a plain string through', () => {
    expect(runFailure('forbidden')).toBe('forbidden')
  })
  it('falls back to the stub when there is nothing to say', () => {
    expect(runFailure(undefined)).toBe('The run could not be started.')
  })
})
