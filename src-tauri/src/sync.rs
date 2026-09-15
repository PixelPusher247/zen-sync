use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::bundle::{self, RestoreReport};
use crate::github::{GitHubClient, MachineMetadata, SnapshotEntry, SyncMetadata};
use crate::{crypto, profile};

/// Pre-restore safety copies kept in the app config dir.
const SAFETY_COPIES_TO_KEEP: usize = 3;

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_iso() -> String {
    epoch_to_iso(now_epoch())
}

fn epoch_to_iso(epoch: u64) -> String {
    let sec = (epoch % 60) as u32;
    let min = ((epoch / 60) % 60) as u32;
    let hour = ((epoch / 3600) % 24) as u32;
    let mut days = (epoch / 86400) as u32;
    let mut y = 1970u32;
    loop {
        let dy = if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let mdays = [31u32, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md { break; }
        days -= md;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{:02}T{hour:02}:{min:02}:{sec:02}Z", days + 1)
}

/// Back up Sine mods and extension data to GitHub.
/// Returns the ISO timestamp of the backup.
pub async fn backup(
    client: &GitHubClient,
    machine_name: &str,
    machine_id: &str,
    max_snapshots: u8,
    overrides: BTreeMap<String, bool>,
    delete_legacy: bool,
    on_progress: impl Fn(&str) + Send,
) -> Result<String, String> {
    on_progress("Checking for old snapshots…");
    let legacy = client.find_legacy_snapshots().await?;
    if !legacy.is_empty() && !delete_legacy {
        return Err(
            "Old full-profile snapshots must be deleted before the first backup in the new format."
                .into(),
        );
    }

    on_progress("Finding Zen profile…");
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    crate::zslog!("[sync] backup: profile at {}", profile_dir.display());

    on_progress("Collecting mods and extension data…");
    let (archive, manifest) =
        tokio::task::spawn_blocking(move || bundle::build(&profile_dir, &overrides))
            .await
            .map_err(|e| format!("Backup task failed: {e}"))??;
    crate::zslog!(
        "[sync] backup: bundle {} bytes, {} extensions",
        archive.len(),
        manifest.extensions.len()
    );

    on_progress("Encrypting…");
    let encrypted = crypto::encrypt(&archive, &client.encryption_key)?;
    crate::zslog!("[sync] backup: encrypted {} bytes", encrypted.len());

    on_progress("Reading current backup list…");
    let (mut metadata, sha) = match client.read_metadata().await? {
        Some(m) => (m.metadata, Some(m.sha)),
        None => (SyncMetadata::default(), None),
    };

    let machine_meta = metadata
        .machines
        .iter_mut()
        .find(|m| m.machine_id == machine_id);

    let (index, expired_indices) = if let Some(mm) = machine_meta {
        let next = mm.next_index(max_snapshots);
        // Collect indices that will be evicted (ring-buffer overflow)
        let expired: Vec<u8> = mm
            .snapshots
            .iter()
            .filter(|s| s.index == next)
            .map(|s| s.index)
            .collect();
        (next, expired)
    } else {
        (0u8, vec![])
    };

    on_progress("Uploading…");
    crate::zslog!("[sync] backup: uploading to slot {index} for machine '{machine_id}'");
    client.upload_snapshot(machine_id, index, &encrypted).await?;

    // Delete any evicted asset
    if !expired_indices.is_empty() {
        on_progress("Pruning old snapshots…");
        client
            .delete_snapshot_assets(machine_id, &expired_indices)
            .await?;
    }

    let pushed_at = now_iso();
    let new_entry = SnapshotEntry {
        index,
        pushed_at: pushed_at.clone(),
        machine_name: machine_name.to_string(),
        size_bytes: encrypted.len() as u64,
    };

    // Update or insert machine metadata
    if let Some(mm) = metadata.machines.iter_mut().find(|m| m.machine_id == machine_id) {
        // Remove existing entry for this slot, push new one at front
        mm.snapshots.retain(|s| s.index != index);
        mm.snapshots.insert(0, new_entry);
        mm.snapshots.truncate(max_snapshots as usize);
        mm.current_index = index;
        mm.max_snapshots = max_snapshots;
    } else {
        metadata.machines.push(MachineMetadata {
            machine_id: machine_id.to_string(),
            snapshots: vec![new_entry],
            max_snapshots,
            current_index: index,
        });
    }

    on_progress("Saving backup list…");
    client.write_metadata(&metadata, sha.as_deref()).await?;

    // Only remove the old format once the new snapshot is safely stored.
    if !legacy.is_empty() {
        on_progress("Deleting old snapshots…");
        client.delete_legacy_snapshots(&legacy).await?;
    }

    crate::zslog!("[sync] backup: done at {pushed_at}");
    Ok(pushed_at)
}

/// Restore a snapshot from GitHub into the local profile.
pub async fn restore(
    client: &GitHubClient,
    machine_id: &str,
    index: u8,
    overrides: BTreeMap<String, bool>,
    safety_root: PathBuf,
    on_progress: impl Fn(&str) + Send,
) -> Result<RestoreReport, String> {
    on_progress("Downloading…");
    crate::zslog!("[sync] restore: fetching machine '{machine_id}' slot {index}");
    let encrypted = client.download_snapshot(machine_id, index).await?;

    on_progress("Decrypting…");
    let archive = crypto::decrypt(&encrypted, &client.encryption_key)?;

    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found.")?;
    crate::zslog!("[sync] restore: writing to {}", profile_dir.display());

    on_progress("Writing files…");
    let safety_dir = safety_root.join(now_epoch().to_string());
    let report = tokio::task::spawn_blocking(move || {
        let result = bundle::apply(&profile_dir, &archive, &overrides, &safety_dir);
        prune_safety_copies(&safety_root);
        result.map_err(|e| {
            if safety_dir.exists() {
                format!("{e}\n\nFiles from before the restore were saved to {}", safety_dir.display())
            } else {
                e
            }
        })
    })
    .await
    .map_err(|e| format!("Restore task failed: {e}"))??;

    crate::zslog!(
        "[sync] restore: {} mods, {} extensions, {} warnings",
        report.mod_count,
        report.extension_count,
        report.warnings.len()
    );
    Ok(report)
}

/// Keep only the newest safety copies. Folder names are epoch seconds.
fn prune_safety_copies(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let excess = dirs.len().saturating_sub(SAFETY_COPIES_TO_KEEP);
    for dir in dirs.into_iter().take(excess) {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => crate::zslog!("[sync] removed old safety copy {}", dir.display()),
            Err(e) => crate::zslog!("[sync] could not remove {}: {e}", dir.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn epoch_to_iso_known() {
        assert_eq!(epoch_to_iso(1779235200), "2026-05-20T00:00:00Z");
    }

    #[test]
    fn prunes_all_but_newest_safety_copies() {
        let root = tempdir().unwrap();
        for ts in ["1789000001", "1789000002", "1789000003", "1789000004"] {
            std::fs::create_dir_all(root.path().join(ts)).unwrap();
        }
        prune_safety_copies(root.path());
        assert!(!root.path().join("1789000001").exists());
        assert!(root.path().join("1789000004").exists());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), SAFETY_COPIES_TO_KEEP);
    }
}
