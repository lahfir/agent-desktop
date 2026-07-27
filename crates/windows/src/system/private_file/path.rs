//! Per-component reparse-point rejection for private-file paths.
//!
//! A junction is creatable by an unprivileged user without
//! `SeCreateSymbolicLinkPrivilege`, and one planted on a private path
//! redirects where the product writes — no ACL on the intended destination
//! prevents a write that never reaches it. Every component of a write path is
//! therefore opened with `FILE_FLAG_OPEN_REPARSE_POINT` (never following the
//! link) and refused if it carries `FILE_ATTRIBUTE_REPARSE_POINT`. This is
//! the Windows analogue of the unix per-component symlink rejection in core's
//! `private_file_parent.rs`, and it restores the check the deleted v0.5.0
//! layer carried. Reads guard only the leaf, exactly as the unix baseline's
//! `O_NOFOLLOW` does.

use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, READ_CONTROL,
};

use super::owner;
use super::{invalid_input, permission_denied};

const NO_FOLLOW_DIRECTORY_FLAGS: u32 = FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS;

#[derive(Clone, Copy, PartialEq, Eq)]
enum MissingComponents {
    Create,
    Reject,
}

pub(super) fn ensure_private_directory_chain(path: &Path) -> std::io::Result<()> {
    walk_directory_components(path, MissingComponents::Create)?;
    let directory = open_directory_no_follow(path, FILE_READ_ATTRIBUTES | READ_CONTROL)?;
    require_verified_directory(&directory)?;
    owner::require_owned_by_token_owner(&directory, "private file parent")
}

pub(super) fn require_reparse_free_directory_chain(path: &Path) -> std::io::Result<()> {
    walk_directory_components(path, MissingComponents::Reject)
}

fn walk_directory_components(path: &Path, missing: MissingComponents) -> std::io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(invalid_input(
                    "private file parent must not contain parent traversal",
                ));
            }
            Component::Prefix(prefix) => {
                current.push(prefix.as_os_str());
                continue;
            }
            Component::RootDir | Component::Normal(_) => current.push(component.as_os_str()),
        }
        verify_directory_component(&current, missing)?;
    }
    Ok(())
}

fn verify_directory_component(
    component_path: &Path,
    missing: MissingComponents,
) -> std::io::Result<()> {
    match open_directory_no_follow(component_path, FILE_READ_ATTRIBUTES) {
        Ok(directory) => require_verified_directory(&directory),
        Err(error)
            if error.kind() == ErrorKind::NotFound && missing == MissingComponents::Create =>
        {
            create_directory_component(component_path)?;
            let directory = open_directory_no_follow(component_path, FILE_READ_ATTRIBUTES)?;
            require_verified_directory(&directory)
        }
        Err(error) => Err(error),
    }
}

fn create_directory_component(component_path: &Path) -> std::io::Result<()> {
    match std::fs::create_dir(component_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn open_directory_no_follow(path: &Path, access: u32) -> std::io::Result<File> {
    OpenOptions::new()
        .access_mode(access)
        .custom_flags(NO_FOLLOW_DIRECTORY_FLAGS)
        .open(path)
}

fn require_verified_directory(directory: &File) -> std::io::Result<()> {
    let attributes = handle_attributes(directory)?;
    require_not_reparse_point(attributes, "private file path component is a reparse point")?;
    if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(permission_denied(
            "private file path component must be a directory",
        ));
    }
    Ok(())
}

pub(super) fn open_leaf_regular_no_follow(
    path: &Path,
    options: &mut OpenOptions,
    what: &str,
) -> std::io::Result<File> {
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path)?;
    require_regular_leaf(&file, what)?;
    Ok(file)
}

pub(super) fn open_leaf_for_validation(path: &Path, what: &str) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES | READ_CONTROL)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    require_regular_leaf(&file, what)?;
    Ok(file)
}

fn require_regular_leaf(file: &File, what: &str) -> std::io::Result<()> {
    let attributes = handle_attributes(file)?;
    require_not_reparse_point(attributes, format!("{what} is a reparse point"))?;
    if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
        return Err(permission_denied(format!("{what} is not a regular file")));
    }
    Ok(())
}

pub(super) fn require_verified_lease_directory(directory: &File) -> std::io::Result<()> {
    require_verified_directory(directory)
}

fn require_not_reparse_point(attributes: u32, message: impl Into<String>) -> std::io::Result<()> {
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(permission_denied(message));
    }
    Ok(())
}

fn handle_attributes(file: &File) -> std::io::Result<u32> {
    Ok(file.metadata()?.file_attributes())
}
