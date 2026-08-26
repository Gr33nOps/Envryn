//! Model download and verification.
//!
//! `docs/AI_SECURITY.md` section 7: "Before a downloaded model is used,
//! Envryn verifies expected file size, a cryptographic checksum pinned in
//! the application, the download source, and the model version. A mismatch
//! deletes the file and reports a failure; Envryn never loads a model that
//! did not verify."
//!
//! **Source is pinned by construction, not checked at download time**:
//! there is no public function here that accepts an arbitrary URL. Callers
//! choose one of the [`ModelSpec`] constants; that is the entire "trusted
//! source" surface. Size and checksum are verified for real, streaming the
//! download so a multi-hundred-megabyte file is never held in memory at
//! once, and a mismatch on either deletes the partially-written file before
//! returning an error -- nothing partially downloaded is ever left where
//! [`crate::ai::worker_client`] would find and load it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// One approved, pinned model. Every field is `'static` and baked into the
/// binary -- there is no code path that constructs one from user input.
#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub display_name: &'static str,
    pub version: &'static str,
    pub arch: &'static str,
    pub eos_token: &'static str,
    pub model_url: &'static str,
    pub model_filename: &'static str,
    pub model_size_bytes: u64,
    pub model_sha256_hex: &'static str,
    pub tokenizer_url: &'static str,
    pub tokenizer_filename: &'static str,
    pub tokenizer_size_bytes: u64,
    pub tokenizer_sha256_hex: &'static str,
}

/// The one Tier-1 model this build knows how to fetch. Stepped up from
/// Qwen2-0.5B-Instruct to Qwen2.5-1.5B-Instruct (3x the parameters, same
/// GGUF architecture family so `envryn-ai-worker`'s loader needed no
/// changes) after real-world use found the 0.5B model's name/classification
/// suggestions too unreliable to trust -- still small enough to stay CPU-only
/// per specification section 51 ("usable without a GPU"), and verified
/// against real inference in `tests/ai_real_model.rs`, not just downloaded
/// and assumed to work. Size and checksum below were computed independently
/// from a real downloaded copy of the file, not copied from Hugging Face's
/// own reported metadata.
pub const QWEN2_5_1_5B_INSTRUCT: ModelSpec = ModelSpec {
    display_name: "Qwen2.5 1.5B Instruct (Q4_0 GGUF)",
    version: "qwen2.5-1.5b-instruct-q4_0-2024-09",
    arch: "qwen2",
    eos_token: "<|im_end|>",
    model_url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_0.gguf",
    model_filename: "qwen2.5-1.5b-instruct-q4_0.gguf",
    model_size_bytes: 1_066_227_232,
    model_sha256_hex: "dcd819ff094852c38faba6873d8ff0c9d51eadb2844539e52042ae5d647bbfdb",
    tokenizer_url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/resolve/main/tokenizer.json",
    tokenizer_filename: "tokenizer.json",
    tokenizer_size_bytes: 7_031_645,
    tokenizer_sha256_hex: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFiles {
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("could not reach the download source")]
    Network,
    #[error("the downloaded file was {actual} bytes, expected {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("the downloaded file's checksum did not match")]
    ChecksumMismatch,
    #[error("could not write the downloaded file to disk")]
    Io,
}

/// Verify an already-present file against the size and checksum pinned in
/// `spec` -- pure and network-free, so this is what this module's own tests
/// exercise directly. [`download_and_verify`] is a thin network-fetching
/// wrapper around this same check.
fn verify_file(
    path: &Path,
    expected_size: u64,
    expected_sha256_hex: &str,
) -> Result<(), DownloadError> {
    let metadata = std::fs::metadata(path).map_err(|_| DownloadError::Io)?;
    if metadata.len() != expected_size {
        return Err(DownloadError::SizeMismatch {
            expected: expected_size,
            actual: metadata.len(),
        });
    }
    let mut file = std::fs::File::open(path).map_err(|_| DownloadError::Io)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|_| DownloadError::Io)?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).ok_or(DownloadError::Io)?;
        hasher.update(chunk);
    }
    let hex = hex_encode(&hasher.finalize());
    if hex != expected_sha256_hex {
        return Err(DownloadError::ChecksumMismatch);
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Bytes fetched so far for one file, and the size expected once complete.
/// Emitted at most a few dozen times per file (throttled below) -- callers
/// (the Tauri shell) turn this into a progress bar; this crate has no
/// knowledge of Tauri or the UI.
#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub file_name: &'static str,
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
}

/// Below this many new bytes, `download_one` does not re-invoke the
/// progress callback -- a 350&nbsp;MB file read in 64&nbsp;KB chunks is
/// ~5,600 chunks; without throttling that is ~5,600 IPC events for one
/// download, chatty for no benefit to a percentage display.
const PROGRESS_STEP_BYTES: u64 = 1024 * 1024;

fn download_one(
    url: &str,
    dest: &Path,
    file_name: &'static str,
    expected_size: u64,
    expected_sha256_hex: &str,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<(), DownloadError> {
    let response = ureq::get(url).call().map_err(|_| DownloadError::Network)?;
    let mut reader = response.into_reader();
    let tmp_path = dest.with_extension("part");
    let cleanup = |path: &Path| {
        let _ = std::fs::remove_file(path);
    };

    {
        let mut file = std::fs::File::create(&tmp_path).map_err(|_| DownloadError::Io)?;
        let mut buf = [0u8; 64 * 1024];
        let mut total: u64 = 0;
        let mut last_reported: u64 = 0;
        on_progress(DownloadProgress {
            file_name,
            bytes_downloaded: 0,
            total_bytes: expected_size,
        });
        loop {
            let n = reader.read(&mut buf).map_err(|_| DownloadError::Network)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > expected_size {
                drop(file);
                cleanup(&tmp_path);
                return Err(DownloadError::SizeMismatch {
                    expected: expected_size,
                    actual: total,
                });
            }
            let chunk = buf.get(..n).ok_or(DownloadError::Io)?;
            file.write_all(chunk).map_err(|_| DownloadError::Io)?;
            if total - last_reported >= PROGRESS_STEP_BYTES {
                last_reported = total;
                on_progress(DownloadProgress {
                    file_name,
                    bytes_downloaded: total,
                    total_bytes: expected_size,
                });
            }
        }
        on_progress(DownloadProgress {
            file_name,
            bytes_downloaded: total,
            total_bytes: expected_size,
        });
    }

    if let Err(e) = verify_file(&tmp_path, expected_size, expected_sha256_hex) {
        cleanup(&tmp_path);
        return Err(e);
    }

    std::fs::rename(&tmp_path, dest).map_err(|_| DownloadError::Io)
}

/// Download and verify both files for `spec` into `dest_dir`, which is
/// created if it does not exist. `docs/AI_SECURITY.md` section 7: "Model
/// files are stored under `/models`, never under `/vault`" -- the caller
/// (the Tauri shell) is responsible for pointing `dest_dir` at the models
/// directory, never at the vault's own storage location, the same rule
/// `src-tauri/src/ipc.rs`'s module doc already states for `backup_create`.
///
/// If either file already exists at its destination with the correct size
/// and checksum, it is reused rather than re-downloaded.
pub fn download_and_verify(spec: &ModelSpec, dest_dir: &Path) -> Result<ModelFiles, DownloadError> {
    download_and_verify_with_progress(spec, dest_dir, &mut |_| {})
}

/// Same as [`download_and_verify`], but calls `on_progress` as bytes arrive
/// so a caller (the Tauri shell) can show real download progress instead of
/// an indeterminate spinner for what is, for the ~350&nbsp;MB model file, a
/// multi-minute wait on an ordinary connection.
pub fn download_and_verify_with_progress(
    spec: &ModelSpec,
    dest_dir: &Path,
    on_progress: &mut dyn FnMut(DownloadProgress),
) -> Result<ModelFiles, DownloadError> {
    std::fs::create_dir_all(dest_dir).map_err(|_| DownloadError::Io)?;
    let model_path = dest_dir.join(spec.model_filename);
    let tokenizer_path = dest_dir.join(spec.tokenizer_filename);

    if verify_file(&model_path, spec.model_size_bytes, spec.model_sha256_hex).is_err() {
        download_one(
            spec.model_url,
            &model_path,
            spec.model_filename,
            spec.model_size_bytes,
            spec.model_sha256_hex,
            on_progress,
        )?;
    }
    if verify_file(
        &tokenizer_path,
        spec.tokenizer_size_bytes,
        spec.tokenizer_sha256_hex,
    )
    .is_err()
    {
        download_one(
            spec.tokenizer_url,
            &tokenizer_path,
            spec.tokenizer_filename,
            spec.tokenizer_size_bytes,
            spec.tokenizer_sha256_hex,
            on_progress,
        )?;
    }

    Ok(ModelFiles {
        model_path,
        tokenizer_path,
    })
}

/// Check whether `spec`'s files are already present and verified at
/// `dest_dir`, without touching the network. Used to answer "is AI ready to
/// use" without triggering a download as a side effect.
pub fn already_verified(spec: &ModelSpec, dest_dir: &Path) -> Option<ModelFiles> {
    let model_path = dest_dir.join(spec.model_filename);
    let tokenizer_path = dest_dir.join(spec.tokenizer_filename);
    verify_file(&model_path, spec.model_size_bytes, spec.model_sha256_hex).ok()?;
    verify_file(
        &tokenizer_path,
        spec.tokenizer_size_bytes,
        spec.tokenizer_sha256_hex,
    )
    .ok()?;
    Some(ModelFiles {
        model_path,
        tokenizer_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex_encode(&hasher.finalize())
    }

    #[test]
    fn verify_file_accepts_a_matching_file() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"a small fake model file";
        let path = write_temp(dir.path(), "model.bin", content);
        let hash = sha256_hex(content);
        assert!(verify_file(&path, content.len() as u64, &hash).is_ok());
    }

    #[test]
    fn verify_file_rejects_a_size_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"a small fake model file";
        let path = write_temp(dir.path(), "model.bin", content);
        let hash = sha256_hex(content);
        let result = verify_file(&path, content.len() as u64 + 1, &hash);
        assert!(matches!(result, Err(DownloadError::SizeMismatch { .. })));
    }

    #[test]
    fn verify_file_rejects_a_checksum_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let content = b"a small fake model file";
        let path = write_temp(dir.path(), "model.bin", content);
        let wrong_hash = sha256_hex(b"a completely different file");
        let result = verify_file(&path, content.len() as u64, &wrong_hash);
        assert!(matches!(result, Err(DownloadError::ChecksumMismatch)));
    }

    #[test]
    fn verify_file_rejects_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist.bin");
        assert!(matches!(
            verify_file(&missing, 10, "irrelevant"),
            Err(DownloadError::Io)
        ));
    }

    /// A tiny stand-in `ModelSpec` so these tests never write or hash the
    /// real (350 MB) model -- only `verify_file`'s logic is under test
    /// here, and it is identical regardless of file size.
    fn tiny_spec() -> ModelSpec {
        ModelSpec {
            display_name: "test",
            version: "test",
            arch: "test",
            eos_token: "<|im_end|>",
            model_url: "https://example.invalid/model.gguf",
            model_filename: "model.gguf",
            model_size_bytes: 4,
            // sha256("test"), verified independently rather than recalled
            model_sha256_hex: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            tokenizer_url: "https://example.invalid/tokenizer.json",
            tokenizer_filename: "tokenizer.json",
            tokenizer_size_bytes: 4,
            tokenizer_sha256_hex:
                "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
        }
    }

    #[test]
    fn already_verified_is_none_when_nothing_has_been_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        assert!(already_verified(&tiny_spec(), dir.path()).is_none());
    }

    #[test]
    fn already_verified_is_none_for_a_tampered_file() {
        let spec = tiny_spec();
        let dir = tempfile::tempdir().unwrap();
        // Right filename, right size (4 bytes), wrong content -- the
        // checksum must still catch this even though the size check alone
        // would not.
        std::fs::write(dir.path().join(spec.model_filename), b"nope").unwrap();
        assert!(already_verified(&spec, dir.path()).is_none());
    }

    #[test]
    fn already_verified_is_some_once_both_files_match() {
        let spec = tiny_spec();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(spec.model_filename), b"test").unwrap();
        std::fs::write(dir.path().join(spec.tokenizer_filename), b"test").unwrap();
        assert!(already_verified(&spec, dir.path()).is_some());
    }

    /// The regression this guards: `ai_download_model`'s frontend button
    /// gave no feedback for the ~170s a real download takes, which is
    /// indistinguishable from "broken" to a user. `download_and_verify`'s
    /// network-free fast path (both files already present and valid) must
    /// also never fire `on_progress` -- there is nothing to report progress
    /// on, and a stray event here would flash a spurious progress bar for
    /// an instant, network-free confirm.
    #[test]
    fn on_progress_is_never_called_when_both_files_already_verify() {
        let spec = tiny_spec();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(spec.model_filename), b"test").unwrap();
        std::fs::write(dir.path().join(spec.tokenizer_filename), b"test").unwrap();

        let mut calls: Vec<DownloadProgress> = Vec::new();
        let result = download_and_verify_with_progress(&spec, dir.path(), &mut |p| calls.push(p));

        assert!(result.is_ok());
        assert!(
            calls.is_empty(),
            "expected no progress events for already-verified files, got {calls:?}"
        );
    }
}
