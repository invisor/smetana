/* What a project is called on a 28px tile. Pure, and a module of its own for
   the reason the `projectMenu.js` family is: no test in this repository can
   reach a `.vue`, so a rule left inside the component that draws it is a rule
   nothing checks.

   Collisions are tolerated rather than resolved. Two projects called `smetana`
   under different parents both draw `sm`, and mangling the second one's letters
   to tell them apart produces a label that means nothing to anybody. The tile's
   tooltip carries the full name, the branch and the state, and the tooltip is
   where a person who is unsure looks. */

/** Two characters at most, one at least, always lower case. */
export function monogram(name) {
  const segments = String(name ?? '')
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
  if (segments.length === 0) return '··'
  const letters =
    segments.length > 1 ? segments[0][0] + segments[1][0] : segments[0].slice(0, 2)
  return letters.toLowerCase()
}
