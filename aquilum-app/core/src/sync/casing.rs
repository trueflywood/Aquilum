use super::apply::{create, remember, stage};
use super::cycle::Vault;
use super::fold::{fold_path, same_fold};
use super::measure::Local;
use super::records::Record;
use super::remote::RemoteFile;
use super::scan::{absolute, service, stamp, LocalFile, UNKNOWN};
use super::view::View;
use super::SyncError;
use crate::files::content::FileContent;
use crate::files::document::hash_file;
use crate::files::gate;
use crate::history::Source;
use std::collections::{BTreeMap, HashMap};
use std::fs;

struct Group<'a> {
    locals: Vec<(&'a str, LocalFile)>,
    remotes: Vec<&'a RemoteFile>,
}

pub fn resolve(vault: &Vault<'_>, local: &mut Local, view: &mut View, records: &mut HashMap<String, Record>) -> Result<(), SyncError> {
    let aliases: Vec<(String, LocalFile, RemoteFile)> = groups(local, view)
        .into_values()
        .filter_map(|group| {
            let (local_path, local_file) = *group.locals.first()?;
            let remote = (*group.remotes.first()?).clone();
            (same_fold(local_path, &remote.path) && local_path != remote.path).then(|| (local_path.to_owned(), local_file, remote))
        })
        .collect();
    for (local_path, local_file, remote) in aliases {
        if local_file.modified >= remote.modified {
            replace_remote(vault, &local_path, local_file, &remote, view, records)?;
        } else {
            replace_local(vault, local, &local_path, &remote, view, records)?;
        }
    }
    Ok(())
}

fn groups<'a>(local: &'a Local, view: &'a View) -> BTreeMap<String, Group<'a>> {
    let mut groups: BTreeMap<String, Group<'_>> = BTreeMap::new();
    for (path, file) in &local.files {
        groups.entry(fold_path(path)).or_insert_with(|| Group { locals: Vec::new(), remotes: Vec::new() }).locals.push((path.as_str(), *file));
    }
    for path in view.paths() {
        if let Some(remote) = view.get(path, None) {
            let group = groups.entry(fold_path(path)).or_insert_with(|| Group { locals: Vec::new(), remotes: Vec::new() });
            if !group.remotes.iter().any(|file| file.path == remote.path) {
                group.remotes.push(remote);
            }
        }
    }
    groups
}

fn replace_remote(
    vault: &Vault<'_>,
    local_path: &str,
    local_file: LocalFile,
    remote: &RemoteFile,
    view: &mut View,
    records: &mut HashMap<String, Record>,
) -> Result<(), SyncError> {
    vault.remote.delete(remote)?;
    view.remove_fold(&remote.path);
    forget_records(vault, records, &remote.path)?;
    let hash = hash_file(&absolute(vault.root, local_path))?;
    let uploaded = vault.remote.upload(local_path, &FileContent::open(&absolute(vault.root, local_path))?, local_file.modified, None)?;
    remember(vault, local_path, hash.clone(), local_file, uploaded.clone())?;
    records.retain(|path, _| !same_fold(path, local_path));
    records.insert(local_path.to_owned(), Record { hash, size: local_file.size, modified: local_file.modified, remote: uploaded.clone() });
    view.insert(uploaded);
    Ok(())
}

fn replace_local(
    vault: &Vault<'_>,
    local: &mut Local,
    local_path: &str,
    remote: &RemoteFile,
    view: &mut View,
    records: &mut HashMap<String, Record>,
) -> Result<(), SyncError> {
    let old = absolute(vault.root, local_path);
    if service(local_path) {
        fs::remove_file(&old)?;
    } else {
        gate::trash(vault.core, vault.root, &old)?;
    }
    local.files.remove(local_path);
    forget_records(vault, records, local_path)?;
    let staged = stage(&vault.net(), remote)?;
    let path = absolute(vault.root, &remote.path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let source = if vault.scope.history { Source::Synced } else { Source::External };
    let written = create(vault, service(&remote.path), &path, &staged.open()?, source)?;
    stamp(&path, remote.modified)?;
    local.files.insert(remote.path.clone(), LocalFile { size: remote.size, modified: remote.modified });
    remember(vault, &remote.path, written.hash.clone(), UNKNOWN, remote.clone())?;
    records.retain(|path, _| !same_fold(path, &remote.path));
    records.insert(remote.path.clone(), Record { hash: written.hash, size: remote.size, modified: remote.modified, remote: remote.clone() });
    view.insert(remote.clone());
    vault.core.synced(vec![path]);
    Ok(())
}

fn forget_records(vault: &Vault<'_>, records: &mut HashMap<String, Record>, path: &str) -> Result<(), SyncError> {
    let doomed: Vec<String> = records.keys().filter(|key| same_fold(key, path)).cloned().collect();
    for key in doomed {
        records.remove(&key);
        vault.records.remove(&key)?;
    }
    Ok(())
}
