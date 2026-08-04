//! Where the bundled skills are, and whether the person already has their own
//! copy of superpowers.

use std::path::PathBuf;

/// The skill library as this run sees it.
pub struct Skills {
    /// Our own skills. Always ours, always present.
    pub smetana: PathBuf,
    /// The vendored superpowers copy. Always a real path: `Inline` and `Auto`
    /// read from it regardless of what is installed.
    pub superpowers: PathBuf,
    /// True when the person has their own superpowers, in which case ours must
    /// not also be handed over.
    pub superpowers_installed: bool,
}
