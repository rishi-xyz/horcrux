//! Detect mounted removable drives (guardians' own USB keys) across
//! Linux/macOS/Windows, via `sysinfo`'s cross-platform `Disks` API.
//!
//! `mount_point` is kept as an opaque [`PathBuf`] deliberately: Linux mounts
//! removable media under `/media/<user>/...` or `/run/media/...`, macOS
//! under `/Volumes/...`, and Windows exposes a drive letter — this module
//! makes no assumption about the shape of any of those paths. Callers just
//! join a filename onto whatever `mount_point` they picked.

use std::path::PathBuf;
use sysinfo::Disks;

/// One mounted drive, as reported by the OS.
#[derive(Debug, Clone)]
pub struct DriveInfo {
    /// Volume label, or the device name if the OS reports no label.
    pub name: String,
    /// Where the drive is mounted; join a filename onto this to write to it.
    pub mount_point: PathBuf,
    /// Whether the OS reports this drive as removable media.
    pub removable: bool,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

/// List every mounted drive the OS reports, removable drives first.
pub fn list_drives() -> Vec<DriveInfo> {
    let disks = Disks::new_with_refreshed_list();
    let mut drives: Vec<DriveInfo> = disks
        .list()
        .iter()
        .map(|d| DriveInfo {
            name: d.name().to_string_lossy().into_owned(),
            mount_point: d.mount_point().to_path_buf(),
            removable: d.is_removable(),
            total_bytes: d.total_space(),
            available_bytes: d.available_space(),
        })
        .collect();
    drives.sort_by(|a, b| {
        b.removable
            .cmp(&a.removable)
            .then_with(|| a.name.cmp(&b.name))
    });
    drives
}

/// List only the drives the OS reports as removable media — the candidates
/// for "this guardian's USB key".
pub fn list_removable_drives() -> Vec<DriveInfo> {
    list_drives().into_iter().filter(|d| d.removable).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Can't assert a removable drive exists in CI, but every returned entry
    /// must at least have a non-empty mount point and the call must not
    /// panic on whatever disks this machine happens to have.
    #[test]
    fn list_drives_does_not_panic_and_reports_sane_mount_points() {
        for drive in list_drives() {
            assert!(!drive.mount_point.as_os_str().is_empty());
        }
    }

    #[test]
    fn list_removable_drives_is_a_subset_of_list_drives_and_all_removable() {
        let all = list_drives();
        let removable = list_removable_drives();
        assert!(removable.len() <= all.len());
        assert!(removable.iter().all(|d| d.removable));
    }
}
