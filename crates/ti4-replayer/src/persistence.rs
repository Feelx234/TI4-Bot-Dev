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

/// SHA-256 of a path, whether it is a file or a bundle.
///
/// The operator's checkpoints are not single files: an MLP inference bundle is a directory holding
/// `slots.json` beside four `.safetensors` files, and R01 records *the directory* as the checkpoint.
/// A project is a promise about the inputs its frames came from, so the promise has to be computable
/// over a bundle too, or opening one of those sessions would refuse at the door.
///
/// A directory digests the sorted listing of what it holds - each entry's name and, for a file, its
/// own digest, walked recursively - under a `dir:` marker at the front of what gets hashed. The marker
/// is what makes the two kinds incomparable: a file and a directory holding exactly that file can
/// never agree on a value, and neither can a bundle with an entry added, removed, renamed or edited.
///
/// # Errors
/// [`ProjectError::MissingInput`] when the path does not exist, [`ProjectError::Io`] when it cannot be
/// read, and [`ProjectError::Malformed`] when a bundle is bigger than [`MAX_BUNDLE_ENTRIES`].
pub fn sha256_path(path: &Path) -> Result<String, ProjectError> {
    let metadata = fs::metadata(path).map_err(|error| {
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
    if !metadata.is_dir() {
        return sha256_file(path);
    }
    let mut listing = String::from("dir:");
    let mut entries = 0_usize;
    digest_dir(path, Path::new(""), &mut listing, &mut entries)?;
    Ok(hex_digest(listing.as_bytes()))
}

/// The most a checkpoint bundle may hold. Generous: a real one is half a dozen files.
const MAX_BUNDLE_ENTRIES: usize = 4_096;

fn digest_dir(
    root: &Path,
    relative: &Path,
    listing: &mut String,
    entries: &mut usize,
) -> Result<(), ProjectError> {
    let mut names: Vec<_> = fs::read_dir(root.join(relative))
        .map_err(|source| ProjectError::Io {
            path: root.join(relative),
            source,
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
        .collect();
    // Filesystem order is not a fact about the bundle. Sorting is what makes the same directory
    // hash to the same value on the next boot, and on the machine next door.
    names.sort();
    for name in names {
        *entries += 1;
        if *entries > MAX_BUNDLE_ENTRIES {
            return Err(ProjectError::Malformed {
                detail: format!(
                    "checkpoint bundle {} holds more than {MAX_BUNDLE_ENTRIES} entries",
                    root.display()
                ),
            });
        }
        let child = relative.join(&name);
        let absolute = root.join(&child);
        let metadata = fs::metadata(&absolute).map_err(|source| ProjectError::Io {
            path: absolute.clone(),
            source,
        })?;
        if metadata.is_dir() {
            let _ = std::fmt::write(listing, format_args!("{}/\n", child.to_string_lossy()));
            digest_dir(root, &child, listing, entries)?;
        } else {
            let digest = sha256_file(&absolute)?;
            let _ = std::fmt::write(
                listing,
                format_args!(
                    "{}:{}
",
                    child.to_string_lossy(),
                    digest
                ),
            );
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A scratch directory, removed when the test ends.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let path = std::env::temp_dir().join(format!(
                "ti4-r02-bundle-{tag}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a scratch directory");
            Self { path }
        }

        fn write(&self, name: &str, body: &str) {
            let target = self.path.join(name);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("a scratch subdirectory");
            }
            fs::write(target, body).expect("a scratch file");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn a_file_hashes_the_same_whichever_way_it_is_asked() {
        let scratch = Scratch::new("file");
        scratch.write("slots.json", "{\"seats\":3}");
        let file = sha256_file(&scratch.path.join("slots.json")).expect("a file hash");
        let path = sha256_path(&scratch.path.join("slots.json")).expect("a path hash");
        assert_eq!(file, path, "a plain file is its own path");
        assert_eq!(file.len(), 64, "a digest, not a summary");
    }

    #[test]
    fn a_bundle_digests_its_contents_and_every_way_they_can_change() {
        let scratch = Scratch::new("bundle");
        scratch.write("slots.json", "{\"seats\":3}");
        scratch.write("trunk.safetensors", "weights");
        let first = sha256_path(&scratch.path).expect("a bundle hash");
        assert!(
            sha256_path(&scratch.path).expect("again") == first,
            "the same bundle must digest the same way twice"
        );

        // An edit anywhere in the bundle is a different bundle.
        scratch.write("trunk.safetensors", "weightt");
        let edited = sha256_path(&scratch.path).expect("after an edit");
        assert_ne!(edited, first, "an edited file is a different checkpoint");

        // So is an extra file, and so is a rename - which is why the names are in the digest.
        let scratch = Scratch::new("extra");
        scratch.write("slots.json", "{\"seats\":3}");
        let alone = sha256_path(&scratch.path).expect("one file");
        scratch.write("extra.json", "{}");
        assert_ne!(
            sha256_path(&scratch.path).expect("two files"),
            alone,
            "a bundle with an extra file is not the bundle that was recorded"
        );
        let scratch = Scratch::new("rename");
        scratch.write("slots.json", "{\"seats\":3}");
        let named = sha256_path(&scratch.path).expect("named");
        fs::rename(
            scratch.path.join("slots.json"),
            scratch.path.join("slotz.json"),
        )
        .expect("a rename");
        assert_ne!(
            sha256_path(&scratch.path).expect("renamed"),
            named,
            "renaming an entry changes the bundle, and says so"
        );

        // Nested entries count too, recursively, so a subdirectory cannot hide a change.
        let scratch = Scratch::new("nested");
        scratch.write("heads/a.json", "one");
        let outer = sha256_path(&scratch.path).expect("nested");
        scratch.write("heads/b.json", "two");
        assert_ne!(
            sha256_path(&scratch.path).expect("deeper"),
            outer,
            "a change below the top level is still a change"
        );
    }

    #[test]
    fn a_directory_and_a_file_cannot_agree_on_a_digest() {
        let scratch = Scratch::new("kind");
        scratch.write("same", "identical bytes");
        let as_file = sha256_file(&scratch.path.join("same")).expect("file digest");
        let as_dir = sha256_path(&scratch.path).expect("directory digest");
        assert_ne!(as_file, as_dir);
        assert_eq!(as_dir.len(), 64, "a directory digest is still a digest");
    }

    #[test]
    fn a_missing_path_is_reported_as_missing() {
        let error = sha256_path(Path::new("Z:/definitely/not/here/r02"))
            .expect_err("nothing is there to hash");
        assert!(
            matches!(error, ProjectError::MissingInput { .. }),
            "a moved checkpoint is a missing input, not a corrupt project: {error}"
        );
    }
}
