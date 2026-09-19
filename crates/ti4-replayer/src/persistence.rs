//! Atomic, bounded reading and writing of a [`ReplayerProject`].
//!
//! The file format is `serde_json` of the project, optionally zstd when the path ends in `.zst`,
//! with a SHA-256 over the payload (checksum field blank) stored inside it. R01 does the same thing
//! for its own sessions and this follows it deliberately: one more format to learn is worse than one
//! more file extension.
//!
//! Writes go to a temporary file, the previous file is renamed aside, the temporary file is renamed
//! into place, and only then is the backup removed. If the swap fails the previous file is restored,
//! so an interrupted save leaves the project that was there before rather than a half-written one.
//! Nothing is written at all when the payload or the project itself is out of bounds.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use sha2::Digest as _;
use sha2::Sha256;

use crate::project::{ProjectError, ReplayerProject};

/// A project file may not exceed R01's session bound. A project stores recipes rather than frames, so
/// this is generous by design; the frame bound in [`crate::project`] is what actually limits one.
pub const MAX_PROJECT_BYTES: usize = ti4_review::MAX_SESSION_BYTES;

/// Write a project atomically, within the byte bound.
///
/// # Errors
/// [`ProjectError::TooLarge`] (nothing is written), a validation refusal from
/// [`ReplayerProject::validate`], or [`ProjectError::Io`] for a file-system failure.
pub fn save_project(path: &Path, project: &ReplayerProject) -> Result<(), ProjectError> {
    project.validate()?;
    let plain = project.payload_bytes()?;
    if plain.len() > MAX_PROJECT_BYTES {
        return Err(ProjectError::TooLarge {
            bytes: plain.len(),
            max: MAX_PROJECT_BYTES,
        });
    }
    let bytes = if compressed(path) {
        zstd::encode_all(plain.as_slice(), 3).map_err(|source| ProjectError::Io {
            path: path.to_owned(),
            source,
        })?
    } else {
        plain
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
    replace_file(path, &bytes)
}

/// Read a project, refusing anything that is not exactly what this build understands.
///
/// # Errors
/// [`ProjectError::Io`], [`ProjectError::TooLarge`], [`ProjectError::Malformed`],
/// [`ProjectError::Unsupported`], [`ProjectError::Checksum`], or a structural refusal from
/// [`ReplayerProject::validate`].
pub fn load_project(path: &Path) -> Result<ReplayerProject, ProjectError> {
    let stored = read_bounded(path)?;
    let bytes = if compressed(path) {
        let decoder = zstd::Decoder::new(stored.as_slice()).map_err(|source| ProjectError::Io {
            path: path.to_owned(),
            source,
        })?;
        let mut bytes = Vec::new();
        decoder
            // Bound the decompressed stream too: a small file must not expand past the limit.
            .take(MAX_PROJECT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| ProjectError::Io {
                path: path.to_owned(),
                source,
            })?;
        if bytes.len() > MAX_PROJECT_BYTES {
            return Err(ProjectError::TooLarge {
                bytes: bytes.len(),
                max: MAX_PROJECT_BYTES,
            });
        }
        bytes
    } else {
        stored
    };
    let project: ReplayerProject =
        serde_json::from_slice(&bytes).map_err(|error| ProjectError::Malformed {
            detail: error.to_string(),
        })?;
    project.validate()?;
    let expected = project.expected_checksum();
    if project.checksum != expected {
        return Err(ProjectError::Checksum {
            expected,
            found: project.checksum.clone(),
        });
    }
    Ok(project)
}

fn compressed(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zst"))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, ProjectError> {
    let mut file = fs::File::open(path).map_err(|source| ProjectError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| ProjectError::Io {
            path: path.to_owned(),
            source,
        })?;
    if bytes.len() > MAX_PROJECT_BYTES {
        return Err(ProjectError::TooLarge {
            bytes: bytes.len(),
            max: MAX_PROJECT_BYTES,
        });
    }
    Ok(bytes)
}

/// Write to a temporary file, keep the previous file as a backup, swap, then drop the backup.
fn replace_file(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let temp = path.with_file_name(format!("{name}.tmp"));
    let backup = path.with_file_name(format!("{name}.bak"));
    fs::write(&temp, bytes).map_err(|source| ProjectError::Io {
        path: temp.clone(),
        source,
    })?;
    if backup.exists() {
        fs::remove_file(&backup).map_err(|source| ProjectError::Io {
            path: backup.clone(),
            source,
        })?;
    }
    let had_old = path.exists();
    if had_old {
        fs::rename(path, &backup).map_err(|source| ProjectError::Io {
            path: path.to_owned(),
            source,
        })?;
    }
    if let Err(source) = fs::rename(&temp, path) {
        if had_old {
            let _ = fs::rename(&backup, path);
        }
        return Err(ProjectError::Io {
            path: path.to_owned(),
            source,
        });
    }
    if had_old {
        let _ = fs::remove_file(&backup);
    }
    Ok(())
}

/// SHA-256 of a file, lower-case hex. Used to pin the inputs a project depends on.
///
/// # Errors
/// [`ProjectError::MissingInput`] when the file is absent, [`ProjectError::Io`] for anything else.
pub fn sha256_file(path: &Path) -> Result<String, ProjectError> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProjectError::MissingInput {
                path: path.to_owned(),
            }
        } else {
            ProjectError::Io {
                path: path.to_owned(),
                source: error,
            }
        }
    })?;
    Ok(hex_digest(&bytes))
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest
        .finalize()
        .iter()
        .fold(String::new(), |mut out, byte| {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        })
}
