//! The Tauri wrapper over the branch reader in `mod.rs`.

use std::path::Path;

use super::{head, Head};

pub fn git_head(project: String) -> Head {
    head(Path::new(&project))
}
