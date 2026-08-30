/* What a rename field selects the moment it opens: the name without its
   extension, which is the part somebody is nearly always changing. VS Code
   selects exactly this, and the reason it matters is that the alternative —
   everything selected — makes the ordinary case, `report.md` to `summary.md`,
   a retype of the extension as well.

   The same split `model::copy_candidates` makes in Rust: the **last** dot, so
   `archive.tar.gz` keeps `.gz` and offers `archive.tar`, and a leading dot is
   part of the name rather than a separator, so `.gitignore` has no extension to
   hold back. The two are deliberately not one implementation and could not be:
   this one is about a text selection in a field and never touches disk, the
   other names a file that is about to exist. */
export function stemRange(name = '') {
  const cut = name.lastIndexOf('.')
  return [0, cut > 0 ? cut : name.length]
}
