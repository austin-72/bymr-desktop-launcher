//! # Save Export Module
//!
//! Writes signed save exports to the player's Downloads folder. The frontend fetches the
//! export from the server; this module only puts it on disk, since the webview can't
//! reliably save files itself on every platform.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tauri::{command, AppHandle, Manager};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

const EXTENSION: &str = "bymsave";

/// Most copies of the same name to try before giving up.
const MAX_ATTEMPTS: u32 = 100;

/// Every export's file name starts with this, so the command can't be used to create
/// arbitrarily named files (including names Windows reserves, like `CON`).
const NAME_PREFIX: &str = "bym-save-";

const MAX_NAME_LENGTH: usize = 80;

/// What every save code starts with.
const CODE_PREFIX: &str = "bymr-save.";

/// Far above any real save code, which is under 200 KB.
const MAX_CODE_BYTES: usize = 2 * 1024 * 1024;

/// Keeps a file name to letters, numbers, `-` and `_`, so it can't point outside the folder.
/// The name must start with `bym-save-` and be a sensible length.
fn sanitize(file_name: &str) -> Option<String> {
    if !file_name.starts_with(NAME_PREFIX) || file_name.len() <= NAME_PREFIX.len() || file_name.len() > MAX_NAME_LENGTH {
        return None;
    }

    Some(
        file_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect(),
    )
}

/// Whether the text looks like a save code: the prefix, then only the characters a code
/// can contain. This keeps the command from writing anything else to disk.
fn is_save_code(contents: &str) -> bool {
    contents.len() <= MAX_CODE_BYTES
        && contents.starts_with(CODE_PREFIX)
        && contents.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// The path for the given attempt: `name.bymsave`, then `name (2).bymsave`, and so on.
fn candidate(dir: &Path, stem: &str, attempt: u32) -> PathBuf {
    match attempt {
        1 => dir.join(format!("{stem}.{EXTENSION}")),
        n => dir.join(format!("{stem} ({n}).{EXTENSION}")),
    }
}

/// The system's Downloads folder. Where the system doesn't name one (some minimal Linux
/// setups have no XDG user dirs), `~/Downloads` if it exists, otherwise the home folder.
fn downloads_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(dir) = app.path().download_dir() {
        return Ok(dir);
    }

    let home = app.path().home_dir().map_err(|err| format!("Could not find your Downloads folder: {err}"))?;
    let downloads = home.join("Downloads");

    Ok(if downloads.is_dir() { downloads } else { home })
}

/// Writes a save export to the player's Downloads folder and returns the full path.
/// An existing file is never overwritten.
///
/// # Arguments
///
/// * `file_name` - The name without an extension; `.bymsave` is added.
/// * `contents` - The save code, written as plain text.
#[command]
pub async fn save_export_file(app: AppHandle, file_name: String, contents: String) -> Result<String, String> {
    let stem = sanitize(&file_name).ok_or("That isn't a valid name for a save file.")?;

    if !is_save_code(&contents) {
        return Err("The server's reply wasn't a save code.".into());
    }

    let dir = downloads_dir(&app)?;

    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|err| format!("Could not open {}: {err}", dir.display()))?;

    for attempt in 1..=MAX_ATTEMPTS {
        let path = candidate(&dir, &stem, attempt);

        // create_new fails if the file exists, so two exports can't race onto one name.
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path).await {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("Could not save {}: {err}", path.display())),
        };

        let written = match file.write_all(contents.as_bytes()).await {
            Ok(()) => file.flush().await,
            Err(err) => Err(err),
        };

        if let Err(err) = written {
            drop(file);
            let _ = tokio::fs::remove_file(&path).await; // Don't leave half a save behind
            return Err(format!("Could not save {}: {err}", path.display()));
        }

        return Ok(path.to_string_lossy().into_owned());
    }

    Err(format!("Too many save files named {stem} in {}. Move some and try again.", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_characters() {
        assert_eq!(sanitize("bym-save-Kyle_1-2026-11-24").as_deref(), Some("bym-save-Kyle_1-2026-11-24"));
        assert_eq!(sanitize("bym-save-../../etc/passwd").as_deref(), Some("bym-save-______etc_passwd"));
        assert_eq!(sanitize("bym-save-a\\b:c").as_deref(), Some("bym-save-a_b_c"));
        assert_eq!(sanitize("../../etc/passwd"), None);
        assert_eq!(sanitize("CON"), None);
        assert_eq!(sanitize("bym-save-"), None);
        assert_eq!(sanitize(&format!("bym-save-{}", "a".repeat(100))), None);
        assert_eq!(sanitize(""), None);
    }

    #[test]
    fn only_save_codes_are_written() {
        assert!(is_save_code("bymr-save.1.c7c1abfccdf3f7be.H4sIAAA_-x.c2ln"));
        assert!(!is_save_code("#!/bin/sh\nrm -rf ~"));
        assert!(!is_save_code("bymr-save.1.abc\n<script>"));
        assert!(!is_save_code(""));
        assert!(!is_save_code(&format!("bymr-save.{}", "A".repeat(MAX_CODE_BYTES))));
    }

    #[test]
    fn candidate_numbers_copies() {
        let dir = Path::new("/tmp");
        assert_eq!(candidate(dir, "save", 1), dir.join("save.bymsave"));
        assert_eq!(candidate(dir, "save", 3), dir.join("save (3).bymsave"));
    }
}
