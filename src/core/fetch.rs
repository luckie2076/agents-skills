//! Fetching sources: shallow git clone + HTTP download + archive extraction.
//!
//! All functions return `tempfile::TempDir`; callers hold it until install finishes
//! (TempDir drops and cleans up automatically).

use std::io::Read;
use std::path::{Path, PathBuf};

use git2::FetchOptions;
use git2::build::{CheckoutBuilder, RepoBuilder};

use crate::error::{Result, SkillsError};

/// Shallow-clone a git repo into a temp dir; checkout `reference` when it is a branch/tag.
pub fn clone_repo(url: &str, reference: Option<&str>) -> Result<tempfile::TempDir> {
    let tmp = tempfile::TempDir::new()?;

    let mut builder = RepoBuilder::new();
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.depth(1);
    builder.fetch_options(fetch_opts);
    if let Some(r) = reference {
        builder.branch(r);
    }
    // The local file:// transport does not support shallow fetch; fall back to non-shallow on failure.
    if builder.clone(url, tmp.path()).is_err() {
        let mut builder = RepoBuilder::new();
        if let Some(r) = reference {
            builder.branch(r);
        }
        builder.clone(url, tmp.path())?;
    }

    // After a shallow clone the branch may not be explicitly checked out; ensure the working tree is usable.
    if let Ok(repo) = git2::Repository::open(tmp.path()) {
        if let Some(r) = reference {
            let _ = checkout_ref(&repo, r);
        }
    }
    Ok(tmp)
}

fn checkout_ref(repo: &git2::Repository, reference: &str) -> Result<()> {
    let mut builder = CheckoutBuilder::new();
    if let Ok(obj) = repo.revparse_single(reference) {
        repo.checkout_tree(&obj, Some(&mut builder))?;
        let _ = repo.set_head_detached(obj.id());
    }
    Ok(())
}

/// Download a URL to a temp file, returning its path and temp dir.
fn download_to_file(url: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let tmp = tempfile::TempDir::new()?;
    let file = tmp.path().join("download");
    let resp = ureq::get(url).call()?;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    std::fs::write(&file, &buf)?;
    Ok((tmp, file))
}

/// Download a URL and extract it (by archive type) into a temp dir, returning the extraction root.
///
/// Supports zip / tar / tar.gz / tgz; single files (e.g. SKILL.md) are written as-is.
pub fn download_and_extract(url: &str) -> Result<(tempfile::TempDir, PathBuf)> {
    let (_file_tmp, file) = download_to_file(url)?;
    let out = tempfile::TempDir::new()?;
    let root = out.path().to_path_buf();

    if is_archive(url, &file) {
        extract_archive(&file, &root)?;
    } else {
        // Single file: copy under root, preserving the original filename.
        let name = file_name_from_url(url);
        std::fs::copy(&file, root.join(name))?;
    }
    Ok((out, root))
}

fn file_name_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("SKILL.md");
    if name.is_empty() {
        "SKILL.md".to_string()
    } else {
        name.to_string()
    }
}

fn is_archive(url: &str, file: &Path) -> bool {
    let lower = url.to_lowercase();
    if lower.contains(".zip") || lower.contains(".tar.gz") || lower.contains(".tgz") {
        return true;
    }
    if lower.contains(".tar") {
        return true;
    }
    // Fallback: sniff the zip magic bytes from content.
    if let Ok(bytes) = std::fs::read(file).map(|b| b) {
        if bytes.len() >= 2 && bytes[0] == b'P' && bytes[1] == b'K' {
            return true;
        }
    }
    false
}

/// Extract an archive into the destination, rejecting `..` path traversal.
fn extract_archive(file: &Path, dest: &Path) -> Result<()> {
    let lower = file.to_string_lossy().to_lowercase();
    if lower.ends_with(".zip") || has_zip_magic(file) {
        extract_zip(file, dest)
    } else {
        extract_tar(file, dest)
    }
}

fn has_zip_magic(file: &Path) -> bool {
    std::fs::read(file)
        .map(|b| b.len() >= 2 && b[0] == b'P' && b[1] == b'K')
        .unwrap_or(false)
}

fn extract_zip(file: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(file)?;
    let mut archive = zip::ZipArchive::new(f)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name() else {
            return Err(SkillsError::msg("Archive contains an unsafe path"));
        };
        let out = safe_join(dest, &name)?;
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }
    Ok(())
}

fn extract_tar(file: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(file)?;
    let is_gz = file.to_string_lossy().to_lowercase().ends_with(".gz")
        || file.to_string_lossy().to_lowercase().ends_with(".tgz");
    if is_gz {
        let decoder = flate2::read::GzDecoder::new(f);
        let mut archive = tar::Archive::new(decoder);
        unpack_tar(&mut archive, dest)?;
    } else {
        let mut archive = tar::Archive::new(f);
        unpack_tar(&mut archive, dest)?;
    }
    Ok(())
}

fn unpack_tar<R: std::io::Read>(archive: &mut tar::Archive<R>, dest: &Path) -> Result<()> {
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let out = safe_join(dest, &path)?;
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&out)?;
        }
    }
    Ok(())
}

/// Reject `..` segments and absolute paths so the extraction target stays within `dest`.
fn safe_join(dest: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(SkillsError::msg("Archive contains an unsafe path"));
    }
    Ok(dest.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_local_repo_via_file_url() {
        // Create a local repo with git2 and commit a SKILL.md.
        let tmp = tempfile::TempDir::new().unwrap();
        let repo_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let repo = git2::Repository::init(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("SKILL.md"),
            "---\nname: test\ndescription: test skill\n---\nbody\n",
        )
        .unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("SKILL.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let url = format!("file://{}", repo_dir.display());
        let cloned = clone_repo(&url, None).unwrap();
        assert!(cloned.path().join("SKILL.md").exists());
    }

    #[test]
    fn download_single_file() {
        // A local file:// cannot be handled directly by ureq; this only verifies the helper for single-file copying.
        assert_eq!(file_name_from_url("https://x/y/SKILL.md?a=b"), "SKILL.md");
        assert_eq!(file_name_from_url("https://x/y/"), "SKILL.md");
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let dest = Path::new("/tmp/x");
        assert!(safe_join(dest, Path::new("../evil")).is_err());
        assert!(safe_join(dest, Path::new("/abs")).is_err());
        assert!(safe_join(dest, Path::new("a/b/c")).is_ok());
    }

    #[test]
    fn extract_zip_with_nested_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let zip_path = tmp.path().join("skill.zip");
        let out = tmp.path().join("out");

        // Build a zip: skill/SKILL.md + skill/scripts/run.sh
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("skill/SKILL.md", opts).unwrap();
            std::io::Write::write_all(&mut zw, b"---\nname: pdf\n---\n").unwrap();
            zw.start_file("skill/scripts/run.sh", opts).unwrap();
            std::io::Write::write_all(&mut zw, b"#!/bin/sh\n").unwrap();
            zw.finish().unwrap();
        }

        extract_zip(&zip_path, &out).unwrap();
        assert!(out.join("skill/SKILL.md").exists());
        assert!(out.join("skill/scripts/run.sh").exists());
    }

    #[test]
    fn extract_tar_gz_with_nested_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let tar_path = tmp.path().join("skill.tar.gz");
        let out = tmp.path().join("out");

        {
            let f = std::fs::File::create(&tar_path).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tw = tar::Builder::new(enc);
            let mut header = tar::Header::new_gnu();
            header.set_size(20);
            header.set_mode(0o644);
            header.set_cksum();
            tw.append_data(
                &mut header,
                "skill/SKILL.md",
                b"---\nname: pdf\n---\n".as_slice(),
            )
            .unwrap();
            tw.finish().unwrap();
        }

        extract_tar(&tar_path, &out).unwrap();
        assert!(out.join("skill/SKILL.md").exists());
    }

    #[test]
    fn extract_zip_rejects_traversal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let zip_path = tmp.path().join("evil.zip");
        let out = tmp.path().join("out");
        std::fs::create_dir_all(&out).unwrap();

        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("../evil.txt", opts).unwrap();
            std::io::Write::write_all(&mut zw, b"evil").unwrap();
            zw.finish().unwrap();
        }

        assert!(extract_zip(&zip_path, &out).is_err());
    }
}
