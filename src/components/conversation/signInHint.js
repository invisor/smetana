/* Whether a line of the journal says the person is signed out, so that the
   panel can offer the harness's own login beside it (`SignInOffer.vue`).

   The whole of the rule, pure: a case-insensitive substring match against a
   short list of phrases. It is deliberately a text match and not a diagnosis.
   Asking the harness whether it is authenticated after every failure would add
   a call to the failure path, and in the case this exists for — a revoked
   refresh token — the stored credentials are still on disk, so such a check can
   answer "yes". An offer of buttons is an action rather than a verdict, and a
   wrong offer breaks nothing, so a phrase is enough.

   Where each phrase comes from:

   - `sign in again`, `log out and sign in`, `refresh token`: Codex's own text
     when its access token cannot be refreshed — "Your access token could not be
     refreshed because your refresh token was revoked. Please log out and sign
     in again." — which arrives as a failed turn or as an `error` notification.
   - `not logged in`, `please run /login`: Claude Code with no credentials.
     Observed on 2026-10-08 with Claude Code 2.1.294 by running
     `claude -p --verbose --output-format stream-json` with `CLAUDE_CONFIG_DIR`
     pointing at an empty folder and no `ANTHROPIC_API_KEY`: it prints a
     synthetic assistant message with the text `Not logged in · Please run
     /login` (flagged `error: "authentication_failed"`), then a `result` event
     with `is_error: true`, the same text in `result`, and exits with status 1.
     The `claude_driver` therefore journals that sentence as the turn's
     failure. Both phrases of it are on the list, so either half is enough.
   - `Invalid API key · Please run /login`: the older wording of the same state,
     also mentioned in `agents/claude.rs`; covered by `please run /login`.
   - `login expired`: from the "Anthropic profile login expired" wording in the
     documentation, which was not observed.

   A phrase that is not here is simply not offered buttons; the person can still
   switch the Smetana UI off and sign in natively, as before. */
const PHRASES = [
  'sign in again',
  'log out and sign in',
  'not logged in',
  'refresh token',
  'please run /login',
  'login expired'
]

/**
 * @param {unknown} text one journal row's text
 * @returns {boolean} whether it reads as a signed-out failure
 */
export function needsSignIn(text) {
  if (typeof text !== 'string' || !text) return false
  const lower = text.toLowerCase()
  return PHRASES.some((phrase) => lower.includes(phrase))
}

/**
 * The key of the one row the offer is drawn under: the last `activity` row in a
 * failed state or `error` row whose text `needsSignIn`. The same sentence often
 * arrives twice in a row — a strip and a line — and buttons under each would be
 * two answers to one question.
 *
 * @param {Array<{key: unknown, kind: string, state?: string, text?: string}>} rows
 * @returns {unknown} the row's `key`, or `null` when no row qualifies
 */
export function signInRowKey(rows) {
  for (let i = rows.length - 1; i >= 0; i -= 1) {
    const row = rows[i]
    const failed = row.kind === 'error' || (row.kind === 'activity' && row.state === 'failed')
    if (failed && needsSignIn(row.text)) return row.key
  }
  return null
}

/* What a sign-in button says. A harness with one variant draws a single plain
   "Sign in"; with several, each says how it signs in. ChatGPT is named for
   Codex's browser variant because that is the account it signs in with, and the
   word would be wrong on any other harness. */
const LABELS = {
  codex: { browser: 'Sign in with ChatGPT' },
  _default: { browser: 'Sign in with a browser', deviceCode: 'Sign in with a device code' }
}

/**
 * @param {string} agent the harness id (`codex`, `claude`)
 * @param {string} variant `browser` or `deviceCode`
 * @param {number} count how many variants this harness offers
 * @returns {string}
 */
export function signInLabel(agent, variant, count) {
  if (count <= 1) return 'Sign in'
  return LABELS[agent]?.[variant] ?? LABELS._default[variant] ?? 'Sign in'
}
