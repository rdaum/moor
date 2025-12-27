// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

//! Workspace scanning and file-to-object mapping.

use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;

/// Scan workspace for .moo files.
pub async fn scan_workspace(workspace: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    scan_directory(workspace, &mut files).await;
    files
}

/// Recursively scan a directory for .moo files.
async fn scan_directory(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(mut entries) = fs::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            Box::pin(scan_directory(&path, files)).await;
        } else if path.extension().is_some_and(|ext| ext == "moo") {
            debug!("Found MOO file: {}", path.display());
            files.push(path);
        }
    }
}
