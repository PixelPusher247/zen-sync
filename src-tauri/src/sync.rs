use std::path::{Path, PathBuf};

use crate::bundle::{self, RestoreReport, Selection};
use crate::crypto::{self, DataKey};
use crate::github::{GitHubClient, MachineMetadata, SnapshotEntry, SyncMetadata};
use crate::profile;

/// Where snapshots go, and the key they are encrypted with.
#[derive(Clone, Copy)]
pub struct Remote<'a> {
    pub client: &'a GitHubClient,
    pub key: &'a DataKey,
}

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

/// How often a backup tries to save the backup list when it keeps changing.
const METADATA_WRITE_ATTEMPTS: u32 = 3;

/// Add a just-uploaded snapshot to this machine's list. Returns the slots
/// that dropped out of it, whose assets can be deleted.
fn record_snapshot(
    metadata: &mut SyncMetadata,
    machine_id: &str,
    entry: SnapshotEntry,
    max_snapshots: u8,
) -> Vec<u8> {
    if let Some(mm) = metadata.machines.iter_mut().find(|m| m.machine_id == machine_id) {
        return mm.record(entry, max_snapshots);
    }
    metadata.machines.push(MachineMetadata {
        machine_id: machine_id.to_string(),
        current_index: entry.index,
        snapshots: vec![entry],
        max_snapshots,
    });
    Vec::new()
}

/// Back up Sine mods and extension data to GitHub.
/// Returns the ISO timestamp of the backup.
pub async fn backup(
    remote: Remote<'_>,
    machine_name: &str,
    machine_id: &str,
    max_snapshots: u8,
    selection: Selection,
    delete_legacy: bool,
    on_progress: impl Fn(&str) + Send,
) -> Result<String, String> {
    let client = remote.client;
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
        tokio::task::spawn_blocking(move || bundle::build(&profile_dir, &selection))
            .await
            .map_err(|e| format!("Backup task failed: {e}"))??;
    crate::zslog!(
        "[sync] backup: bundle {} bytes, {} extensions",
        archive.len(),
        manifest.extensions.len()
    );

    on_progress("Encrypting…");
    let encrypted = crypto::encrypt(&archive, remote.key)?;
    crate::zslog!("[sync] backup: encrypted {} bytes", encrypted.len());

    on_progress("Reading current backup list…");
    let (mut metadata, sha) = match client.read_metadata().await? {
        Some(m) => (m.metadata, Some(m.sha)),
        None => (SyncMetadata::default(), None),
    };

    let index = metadata
        .machines
        .iter()
        .find(|m| m.machine_id == machine_id)
        .map_or(0, |m| m.next_index(max_snapshots));

    on_progress("Uploading…");
    crate::zslog!("[sync] backup: uploading to slot {index} for machine '{machine_id}'");
    // Replaces the asset already in this slot, if any.
    client.upload_snapshot(machine_id, index, &encrypted).await?;

    let pushed_at = now_iso();
    let new_entry = SnapshotEntry {
        index,
        pushed_at: pushed_at.clone(),
        machine_name: machine_name.to_string(),
        size_bytes: encrypted.len() as u64,
    };

    on_progress("Saving backup list…");
    // Another device may save the list between our read and write. GitHub
    // then refuses the write, and the entry is applied to a fresh copy.
    let mut sha = sha;
    let mut attempts = 0;
    let dropped = loop {
        let dropped = record_snapshot(&mut metadata, machine_id, new_entry.clone(), max_snapshots);
        if client.write_metadata(&metadata, sha.as_deref()).await? {
            break dropped;
        }
        attempts += 1;
        if attempts >= METADATA_WRITE_ATTEMPTS {
            return Err(
                "The backup was uploaded, but the backup list kept changing on GitHub and couldn't be saved. Back up again."
                    .into(),
            );
        }
        (metadata, sha) = match client.read_metadata().await? {
            Some(m) => (m.metadata, Some(m.sha)),
            None => (SyncMetadata::default(), None),
        };
    };

    // Snapshots that fell out of the list, e.g. after "Snapshots to keep" was
    // lowered. Removed only once the list no longer points at them.
    if !dropped.is_empty() {
        on_progress("Pruning old snapshots…");
        client.delete_snapshot_assets(machine_id, &dropped).await?;
    }

    // Only remove the old format once the new snapshot is safely stored.
    if !legacy.is_empty() {
        on_progress("Deleting old snapshots…");
        client.delete_legacy_snapshots(&legacy).await?;
    }

    crate::zslog!("[sync] backup: done at {pushed_at}");
    Ok(pushed_at)
}

/// Restore a snapshot from GitHub into the local profile.
///
/// `expected_pushed_at` is the time of the snapshot the user picked. Slots are
/// reused, so if the slot now holds a different snapshot the restore stops
/// instead of applying something the user didn't choose.
pub async fn restore(
    remote: Remote<'_>,
    machine_id: &str,
    index: u8,
    expected_pushed_at: Option<&str>,
    selection: Selection,
    safety_root: PathBuf,
    on_progress: impl Fn(&str) + Send,
) -> Result<RestoreReport, String> {
    let client = remote.client;
    if let Some(expected) = expected_pushed_at {
        on_progress("Checking the backup list…");
        let current = client.read_metadata().await?.and_then(|m| {
            m.metadata
                .machines
                .into_iter()
                .find(|mm| mm.machine_id == machine_id)?
                .snapshots
                .into_iter()
                .find(|s| s.index == index)
                .map(|s| s.pushed_at)
        });
        if current.as_deref() != Some(expected) {
            return Err(
                "This snapshot was replaced by a newer backup since the list was loaded. Refresh History and pick again."
                    .into(),
            );
        }
    }

    on_progress("Downloading…");
    crate::zslog!("[sync] restore: fetching machine '{machine_id}' slot {index}");
    let encrypted = client.download_snapshot(machine_id, index).await?;

    on_progress("Decrypting…");
    let archive = crypto::decrypt(&encrypted, remote.key)?;

    // Zen may have been opened while the snapshot downloaded.
    if crate::zen_check::is_running() {
        return Err("Zen Browser was opened during the restore. Close it and try again; nothing was changed.".into());
    }

    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found.")?;
    crate::zslog!("[sync] restore: writing to {}", profile_dir.display());

    on_progress("Writing files…");
    let safety_dir = safety_root.join(now_epoch().to_string());
    let report = tokio::task::spawn_blocking(move || {
        let result = bundle::apply(&profile_dir, &archive, &selection, &safety_dir);
        prune_safety_copies(&safety_root);
        result.map_err(|e| {
            if safety_dir.exists() {
                format!("{e}\n\nA copy of the files from before the restore is in {}", safety_dir.display())
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

    fn entry(index: u8) -> SnapshotEntry {
        SnapshotEntry {
            index,
            pushed_at: format!("2026-11-0{}T00:00:00Z", index + 1),
            machine_name: "PC".into(),
            size_bytes: 1,
        }
    }

    /// Runs `count` backups the way `backup` does and returns the slots it
    /// would delete after each one.
    fn simulate(mm: &mut MachineMetadata, count: usize, max: u8) -> Vec<Vec<u8>> {
        (0..count)
            .map(|_| {
                let index = mm.next_index(max);
                mm.record(entry(index), max)
            })
            .collect()
    }

    fn slots(mm: &MachineMetadata) -> Vec<u8> {
        mm.snapshots.iter().map(|s| s.index).collect()
    }

    #[test]
    fn wrapping_around_never_deletes_the_new_snapshot() {
        let mut mm = MachineMetadata {
            machine_id: "pc".into(),
            snapshots: vec![],
            max_snapshots: 3,
            current_index: 0,
        };
        mm.record(entry(0), 3);
        // Fill the other two slots, then wrap around twice.
        let deleted = simulate(&mut mm, 2 + 6, 3);
        // Each slot is overwritten in place, so nothing is ever deleted.
        assert!(deleted.iter().all(Vec::is_empty), "{deleted:?}");
        assert_eq!(slots(&mm).len(), 3);
        assert_eq!(slots(&mm)[0], mm.current_index);
    }

    #[test]
    fn lowering_the_limit_deletes_only_dropped_slots() {
        let mut mm = MachineMetadata {
            machine_id: "pc".into(),
            snapshots: vec![entry(2), entry(1), entry(0)],
            max_snapshots: 3,
            current_index: 2,
        };
        let index = mm.next_index(2);
        let deleted = mm.record(entry(index), 2);
        assert!(!deleted.contains(&index));
        assert!(deleted.iter().all(|i| !slots(&mm).contains(i)));
        assert_eq!(slots(&mm).len(), 2);
        assert_eq!(deleted.len(), 1);
    }

    #[test]
    fn raising_the_limit_again_uses_free_slots() {
        let mut mm = MachineMetadata {
            machine_id: "pc".into(),
            snapshots: vec![entry(2), entry(1), entry(0)],
            max_snapshots: 3,
            current_index: 2,
        };
        // Lower to 2: one slot drops out.
        simulate(&mut mm, 1, 2);
        assert_eq!(slots(&mm).len(), 2);
        // Raise back to 3: the next backups fill the free slot, then rotate,
        // instead of overwriting the newest snapshot every time.
        let mut written = Vec::new();
        for _ in 0..3 {
            let index = mm.next_index(3);
            written.push(index);
            mm.record(entry(index), 3);
        }
        assert_eq!(slots(&mm).len(), 3);
        let mut distinct = written.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), 3, "slots written: {written:?}");
    }

    #[test]
    fn a_new_entry_is_recorded_for_a_new_machine() {
        let mut metadata = SyncMetadata::default();
        let dropped = record_snapshot(&mut metadata, "pc", entry(0), 3);
        assert!(dropped.is_empty());
        assert_eq!(metadata.machines.len(), 1);
        assert_eq!(metadata.machines[0].current_index, 0);
    }

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
