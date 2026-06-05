use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::github::{GitHubClient, MachineMetadata, SnapshotEntry, SyncMetadata};
use crate::{crypto, extensions, prefs, profile};

#[derive(Serialize, Deserialize)]
struct SyncBundle {
    version: u8,
    created_at: u64,
    /// filename → base64-encoded bytes
    files: HashMap<String, String>,
}

fn checkpoint_wal(db_path: &Path) -> Result<(), String> {
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .map_err(|e| format!("Could not open {}: {e}", db_path.display()))?;

    let (busy, log, checkpointed): (i64, i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(|e| format!("WAL checkpoint failed: {e}"))?;

    crate::zslog!(
        "[sync] WAL checkpoint: {checkpointed}/{log} pages written (busy_readers={busy})"
    );
    Ok(())
}

fn collect_files(
    profile_dir: &Path,
    selected_ext_ids: &[String],
) -> Result<HashMap<String, String>, String> {
    let mut files = HashMap::new();
    for &name in profile::SYNC_FILES {
        let path = profile_dir.join(name);
        if !path.exists() {
            crate::zslog!("[sync] skipping missing: {name}");
            continue;
        }
        let bytes = match name {
            "prefs.js" => {
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| format!("Could not read prefs.js: {e}"))?;
                prefs::strip_machine_prefs(&content).into_bytes()
            }
            "extensions.json" => {
                let raw = std::fs::read(&path)
                    .map_err(|e| format!("Could not read extensions.json: {e}"))?;
                extensions::filter_extensions(&raw, selected_ext_ids)
                    .map_err(|e| format!("extensions.json filter failed: {e}"))?
            }
            _ => std::fs::read(&path)
                .map_err(|e| format!("Could not read {name}: {e}"))?,
        };
        crate::zslog!("[sync] collected {name} ({} bytes)", bytes.len());
        files.insert(name.to_string(), BASE64.encode(&bytes));
    }
    if files.is_empty() {
        return Err("No sync files found in the Zen profile folder".into());
    }
    Ok(files)
}

fn write_files(
    profile_dir: &Path,
    bundle: &SyncBundle,
    selected_ext_ids: &[String],
) -> Result<Vec<String>, String> {
    let file_names: Vec<String> = bundle.files.keys().cloned().collect();
    local_backup(profile_dir, &file_names)?;

    let mut written = Vec::new();
    for (name, b64) in &bundle.files {
        let bytes = BASE64
            .decode(b64)
            .map_err(|e| format!("Failed to decode {name}: {e}"))?;
        let dest = profile_dir.join(name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir for {name}: {e}"))?;
        }

        let final_bytes = match name.as_str() {
            "prefs.js" => {
                // Re-inject per-machine keys so the local device identity is preserved.
                let local_content = if dest.exists() {
                    std::fs::read_to_string(&dest).unwrap_or_default()
                } else {
                    String::new()
                };
                let restored_content = String::from_utf8_lossy(&bytes).into_owned();
                prefs::restore_machine_prefs(&restored_content, &local_content).into_bytes()
            }
            "extensions.json" => {
                // Merge: only update selected extensions, preserve local ones.
                let local_bytes = if dest.exists() {
                    std::fs::read(&dest).unwrap_or_default()
                } else {
                    bytes.clone()
                };
                if local_bytes.is_empty() {
                    bytes
                } else {
                    extensions::merge_extensions(&bytes, &local_bytes, selected_ext_ids)
                        .unwrap_or(bytes)
                }
            }
            _ => bytes,
        };

        std::fs::write(&dest, &final_bytes)
            .map_err(|e| format!("Failed to write {name}: {e}"))?;

        // Remove SQLite WAL/SHM files so the browser re-opens the DB cleanly.
        if name.ends_with(".sqlite") {
            let _ = std::fs::remove_file(profile_dir.join(format!("{name}-wal")));
            let _ = std::fs::remove_file(profile_dir.join(format!("{name}-shm")));
        }
        written.push(name.clone());
    }
    written.sort();
    Ok(written)
}

/// Create a timestamped local backup of the listed profile files before overwriting.
fn local_backup(profile_dir: &Path, file_names: &[String]) -> Result<(), String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_dir = profile_dir.join(format!("zen-sync-backup-{ts}"));
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create local backup dir: {e}"))?;
    for name in file_names {
        let src = profile_dir.join(name);
        if src.exists() {
            let dest = backup_dir.join(name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create backup subdir: {e}"))?;
            }
            std::fs::copy(&src, dest)
                .map_err(|e| format!("Failed to backup {name}: {e}"))?;
        }
    }
    crate::zslog!("[sync] local backup created at {}", backup_dir.display());
    Ok(())
}

fn now_iso() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    epoch_to_iso(s)
}

fn epoch_to_iso(epoch: u64) -> String {
    let sec = (epoch % 60) as u32;
    let min = ((epoch / 60) % 60) as u32;
    let hour = ((epoch / 3600) % 24) as u32;
    let mut days = (epoch / 86400) as u32;
    let mut y = 1970u32;
    loop {
        let dy = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mdays = [31u32, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1u32;
    for &md in &mdays {
        if days < md { break; }
        days -= md;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{:02}T{hour:02}:{min:02}:{sec:02}Z", days + 1)
}

/// Back up the current profile to GitHub.
/// Returns the ISO timestamp of the backup.
pub async fn backup(
    client: &GitHubClient,
    machine_name: &str,
    machine_id: &str,
    max_snapshots: u8,
    selected_ext_ids: &[String],
    on_progress: impl Fn(&str) + Send,
) -> Result<String, String> {
    on_progress("Finding Zen profile…");
    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found. Is Zen Browser installed?")?;
    crate::zslog!("[sync] backup: profile at {}", profile_dir.display());

    let places_path = profile_dir.join("places.sqlite");
    if places_path.exists() {
        on_progress("Checkpointing database…");
        checkpoint_wal(&places_path)?;
    }

    on_progress("Collecting files…");
    let files = collect_files(&profile_dir, selected_ext_ids)?;

    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    on_progress("Encrypting…");
    let bundle = SyncBundle { version: 1, created_at, files };
    let json = serde_json::to_vec(&bundle).map_err(|e| e.to_string())?;
    let encrypted = crypto::encrypt(&json, &client.encryption_key)?;
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
    client.upload_profile(machine_id, index, &encrypted).await?;

    // Delete any evicted asset
    if !expired_indices.is_empty() {
        on_progress("Pruning old snapshots…");
        client
            .delete_profile_assets(machine_id, &expired_indices)
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
    crate::zslog!("[sync] backup: done at {pushed_at}");
    Ok(pushed_at)
}

/// Restore a profile snapshot from GitHub.
pub async fn restore(
    client: &GitHubClient,
    machine_id: &str,
    index: u8,
    selected_ext_ids: &[String],
    on_progress: impl Fn(&str) + Send,
) -> Result<Vec<String>, String> {
    on_progress("Downloading…");
    crate::zslog!("[sync] restore: fetching machine '{machine_id}' slot {index}");
    let encrypted = client.download_profile(machine_id, index).await?;

    on_progress("Decrypting…");
    let json = crypto::decrypt(&encrypted, &client.encryption_key)?;

    let bundle: SyncBundle = serde_json::from_slice(&json)
        .map_err(|e| format!("Bundle format error: {e}"))?;
    crate::zslog!(
        "[sync] restore: bundle v{}, {} files",
        bundle.version,
        bundle.files.len()
    );

    if bundle.version != 1 {
        return Err(format!(
            "Unsupported bundle version {} — update zen-sync",
            bundle.version
        ));
    }

    let profile_dir = profile::find_zen_profile()
        .ok_or("Zen profile folder not found.")?;
    crate::zslog!("[sync] restore: writing to {}", profile_dir.display());

    on_progress("Writing files…");
    let written = write_files(&profile_dir, &bundle, selected_ext_ids)?;
    crate::zslog!("[sync] restore: wrote {} files", written.len());
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_to_iso_known() {
        assert_eq!(epoch_to_iso(1779235200), "2026-05-20T00:00:00Z");
    }
}
