use super::{Device, FakeRemote};
use crate::sync::SyncError;

#[test]
fn local_case_duplicates_block_the_pass() {
    let remote = FakeRemote::default();
    let device = Device::new();
    device.write("Note.md", "один");
    device.write("note.md", "два");

    match device.try_sync(&remote) {
        Err(SyncError::CaseConflict { paths }) => assert_eq!(paths.len(), 2),
        Ok(_) => panic!("expected case conflict"),
        Err(error) => panic!("expected case conflict, got {error}"),
    }
    assert!(remote.paths().is_empty());
}

#[test]
fn newer_local_casing_replaces_the_cloud_file() {
    let remote = FakeRemote::default();
    let device = Device::new();
    device.write("note.md", "старое");
    device.set_modified("note.md", 1_000);
    device.sync(&remote);
    device.delete("note.md");
    device.write("Note.md", "новое");
    device.set_modified("Note.md", 2_000);

    assert!(device.sync(&remote).failed.is_empty());

    assert_eq!(remote.paths(), vec!["Note.md".to_owned()]);
    assert_eq!(remote.text("Note.md").as_deref(), Some("новое"));
    assert_eq!(device.read("Note.md").as_deref(), Some("новое"));
    assert!(device.read("note.md").is_none());
}

#[test]
fn newer_remote_casing_replaces_the_local_file() {
    let remote = FakeRemote::default();
    remote.put_at("note.md", "облачное", 3_000);
    let device = Device::new();
    device.write("Note.md", "локальное");
    device.set_modified("Note.md", 1_000);

    assert!(device.sync(&remote).failed.is_empty());

    assert_eq!(device.read("note.md").as_deref(), Some("облачное"));
    assert!(device.read("Note.md").is_none());
    assert!(device.trashed());
    assert_eq!(remote.paths(), vec!["note.md".to_owned()]);
}
