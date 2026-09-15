use std::path::{Component, Path, PathBuf};

pub struct WalkEntry {
    /// Path relative to the walk root, using forward slashes.
    pub rel: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Recursively list every directory and regular file under `root`, parents
/// before their contents. Symlinks are skipped.
pub fn walk(root: &Path) -> Vec<WalkEntry> {
    let mut out = Vec::new();
    walk_into(root, root, &mut out);
    out
}

fn walk_into(dir: &Path, root: &Path, out: &mut Vec<WalkEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let Ok(rel) = path.strip_prefix(root) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if meta.is_dir() {
            out.push(WalkEntry { rel, path: path.clone(), is_dir: true });
            walk_into(&path, root, out);
        } else if meta.is_file() {
            out.push(WalkEntry { rel, path, is_dir: false });
        }
    }
}

/// Total size in bytes of all files under `root`.
pub fn dir_size(root: &Path) -> u64 {
    walk(root)
        .iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| std::fs::metadata(&e.path).ok())
        .map(|m| m.len())
        .sum()
}

/// Copy `src` (a file or directory tree) to `dst`, creating parents as needed.
pub fn copy_all(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        std::fs::copy(src, dst)
            .map_err(|e| format!("Failed to copy {}: {e}", src.display()))?;
        return Ok(());
    }
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create {}: {e}", dst.display()))?;
    for entry in walk(src) {
        let target = dst.join(&entry.rel);
        if entry.is_dir {
            std::fs::create_dir_all(&target)
                .map_err(|e| format!("Failed to create {}: {e}", target.display()))?;
        } else {
            std::fs::copy(&entry.path, &target)
                .map_err(|e| format!("Failed to copy {}: {e}", entry.path.display()))?;
        }
    }
    Ok(())
}

/// Join a forward-slash relative path from a bundle onto `base`, rejecting
/// anything that could escape it (`..`, absolute paths, drive prefixes).
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    let mut pushed = false;
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(c)), None) => {
                out.push(c);
                pushed = true;
            }
            _ => return None,
        }
    }
    pushed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn safe_join_accepts_nested_paths() {
        let base = Path::new("base");
        let joined = safe_join(base, "a/[Config] b/c.json").unwrap();
        assert_eq!(joined, base.join("a").join("[Config] b").join("c.json"));
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let base = Path::new("base");
        assert!(safe_join(base, "../evil").is_none());
        assert!(safe_join(base, "a/../../evil").is_none());
        assert!(safe_join(base, "C:/Windows").is_none());
        assert!(safe_join(base, "a\\..\\evil").is_none());
        assert!(safe_join(base, "").is_none());
    }

    #[test]
    fn walk_lists_dirs_and_files_and_copy_round_trips() {
        let src = tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("a/empty")).unwrap();
        std::fs::write(src.path().join("a/file.txt"), b"hello").unwrap();

        let rels: Vec<(String, bool)> =
            walk(src.path()).into_iter().map(|e| (e.rel, e.is_dir)).collect();
        assert_eq!(
            rels,
            vec![
                ("a".to_string(), true),
                ("a/empty".to_string(), true),
                ("a/file.txt".to_string(), false),
            ]
        );
        assert_eq!(dir_size(src.path()), 5);

        let dst = tempdir().unwrap();
        copy_all(src.path(), &dst.path().join("copy")).unwrap();
        assert!(dst.path().join("copy/a/empty").is_dir());
        assert_eq!(std::fs::read(dst.path().join("copy/a/file.txt")).unwrap(), b"hello");
    }
}
