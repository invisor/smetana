/* This is the brief's fallback text, not a real capture: driving an
   interactive TUI from an agent session isn't possible here. Needed for the
   same reason as MOCK_TREE — there's no backend in the browser, but the
   terminal still has to be visible in a tab and in the gallery. Scheduled
   for replacement with a genuine `script`-recorded session during manual
   verification.

   The box's row widths are hand-budgeted by counting characters, not
   measured against xterm's actual width table — whoever replaces this with
   a real capture should not trust them either. One glyph already needed a
   correction: ❯ (U+276F) renders as a single cell in xterm, and a row that
   assumed otherwise came out one column short, with its right border
   sitting visibly left of every other row's. */
const E = ''
export const MOCK_SESSION_OUTPUT =
  `${E}[2m> ${E}[0mrename the worktree when the branch changes\r\n\r\n` +
  `${E}[38;5;208m●${E}[0m Reading ${E}[1msrc/stores/tabs.js${E}[0m\r\n` +
  `  ${E}[32m✓${E}[0m 41 lines\r\n\r\n` +
  `${E}[2m╭──────────────────────────────────────────────────╮${E}[0m\r\n` +
  `${E}[2m│${E}[0m ${E}[1mEdit file${E}[0m                                        ${E}[2m│${E}[0m\r\n` +
  `${E}[2m│${E}[0m                                                  ${E}[2m│${E}[0m\r\n` +
  `${E}[2m│${E}[0m Do you want to make this edit to tabs.js?        ${E}[2m│${E}[0m\r\n` +
  `${E}[2m│${E}[0m                                                  ${E}[2m│${E}[0m\r\n` +
  `${E}[2m│${E}[0m ${E}[36m❯${E}[0m 1. Yes                                         ${E}[2m│${E}[0m\r\n` +
  `${E}[2m│${E}[0m   2. Yes, and don't ask again this session       ${E}[2m│${E}[0m\r\n` +
  `${E}[2m│${E}[0m   3. No, and tell Claude what to do differently  ${E}[2m│${E}[0m\r\n` +
  `${E}[2m╰──────────────────────────────────────────────────╯${E}[0m\r\n`
