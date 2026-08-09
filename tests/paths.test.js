import { describe, expect, it } from 'vitest'
import { basename } from '../src/paths.js'

describe('what a path is called', () => {
  it('is the last segment', () => {
    expect(basename('/Users/someone/Projects/smetana')).toBe('smetana')
    expect(basename('/Users/someone/Projects/smetana/src/App.vue')).toBe('App.vue')
  })

  it('a trailing separator changes nothing', () => {
    // Paths reach this from settings, from the OS dialog and from the run
    // worker, and only some of those normalise.
    expect(basename('/Users/someone/Projects/smetana/')).toBe('smetana')
    expect(basename('/Users/someone/Projects/smetana///')).toBe('smetana')
  })

  it('splits on the Windows separator too, since WebView2 is a target webview', () => {
    // Without this the whole path became the project's name on Windows.
    expect(basename('C:\\Users\\someone\\smetana')).toBe('smetana')
    expect(basename('C:\\Users\\someone\\smetana\\')).toBe('smetana')
  })

  it('a path with nothing left after the separators keeps its own name', () => {
    // A name is more use than an empty gap in a sentence: the run dialog's
    // tooltip interpolates this into "The run in X is driving the browser",
    // and a fourth copy of this rule answering '' here is what put an empty
    // gap on screen.
    expect(basename('/')).toBe('/')
    expect(basename('smetana')).toBe('smetana')
  })
})
