//! Windows hardening for core's five private-file primitives.
//!
//! Four measured behaviors drive four modules: per-component reparse-point
//! rejection (`path`), `ReplaceFileW`-based atomic promotion with a
//! write-scoped temp lease (`replace`), owner-eligible-SID foreign-principal
//! detection (`owner`), and control-call-disciplined storage locality
//! (`locality`). Each override mirrors the portable default's observable
//! semantics — parent handling, create/append/lock open modes, the hashed
//! nonce temp naming, and the bounded-read limits — and adds only the
//! hardening the measured evidence justifies.
//!
//! Deliberately absent: descriptor authoring and DACL validation. A freshly
//! created leaf under the user profile inherits its parent's ACL —
//! `NT AUTHORITY\SYSTEM`, `BUILTIN\Administrators`, and the user, with no
//! `BUILTIN\Users` — while a `ReplaceFileW` rewrite instead keeps the DACL of
//! the leaf it replaces: for our own artifacts the same SYSTEM/Administrators/
//! user grants with no `BUILTIN\Users`, though re-materialized as explicit
//! ACEs rather than inherited ones. Either way the security-relevant grants
//! carry across, so authoring would re-state what Windows grants and
//! validating would re-check it with exactly the ACE-parsing code whose
//! `AceSize` handling sank the deleted v0.5.0 layer. Nothing here calls the
//! ACL/ACE family, a test pins that absence, and a structural test pins the
//! inherited grants on the freshly created leaf and the same principals'
//! survival across the replace path so an OS change breaks a test rather than
//! the product.
//!
//! Locality gates only the write surfaces (atomic writes, appends, lock
//! opens); reads stay ungated so observation commands keep working wherever
//! a readable artifact lives. Ownership checks accept the owner-eligible SID
//! set derived from the process token — `TokenUser`, `TokenOwner`, and any
//! `TokenGroups` entry flagged `SE_GROUP_OWNER` — because a single human's
//! `TokenOwner` flips between their own SID and `BUILTIN\Administrators` as
//! elevation changes, and comparing against `TokenOwner` alone would refuse a
//! path that same human created at a different elevation. They detect
//! pre-creation by a genuinely foreign principal and are not, and cannot be,
//! an isolation boundary between administrators.

mod locality;
pub(crate) mod owner;
mod path;
mod replace;

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

use agent_desktop_core::{PrivateFileOps, bounded_read};

/// Windows implementation of core's private-file seam, installed once per
/// process by the binary and FFI entry points.
#[derive(Debug, Default)]
pub struct WindowsPrivateFile;

impl WindowsPrivateFile {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PrivateFileOps for WindowsPrivateFile {
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| invalid_input("private file path has no parent"))?;
        let destination_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid_input("private file path has an invalid filename"))?;
        let _chain = path::ensure_private_directory_chain(parent)?;
        validate_destination_if_present(path)?;
        let lease = replace::acquire_write_lease(parent)?;
        let (temporary, file) = replace::create_private_temp_file(&lease, destination_name)?;
        write_all_and_sync(file, bytes)?;
        replace::promote_temp_to_destination(path, &temporary)?;
        validate_written_destination(path)
    }

    fn open_private_append(&self, path: &Path) -> std::io::Result<File> {
        let _chain = path
            .parent()
            .map(path::require_reparse_free_directory_chain)
            .transpose()?;
        let mut options = OpenOptions::new();
        options.read(true).create(true).append(true);
        let file = path::open_leaf_regular_no_follow(path, &mut options, "private append target")?;
        owner::require_owned_by_eligible_principal(&file, "private append target")?;
        locality::require_local_for_private_write(&file, "private append target")?;
        Ok(file)
    }

    fn open_private_lock(&self, path: &Path, create: bool) -> std::io::Result<File> {
        let parent = path
            .parent()
            .ok_or_else(|| invalid_input("private file path has no parent"))?;
        let _chain = path::ensure_private_directory_chain(parent)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(create);
        let file = path::open_leaf_regular_no_follow(path, &mut options, "private lock file")?;
        owner::require_owned_by_eligible_principal(&file, "private lock file")?;
        locality::require_local_for_private_write(&file, "private lock file")?;
        Ok(file)
    }

    fn read_private_bounded(&self, path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
        let mut options = OpenOptions::new();
        options.read(true);
        let file = path::open_leaf_regular_no_follow(path, &mut options, "private file")?;
        owner::require_owned_by_eligible_principal(&file, "private file")?;
        bounded_read(file, max_bytes)
    }

    fn ensure_private(&self, path: &Path) -> std::io::Result<()> {
        path::ensure_private_directory_chain(path)?;
        Ok(())
    }
}

fn validate_destination_if_present(path: &Path) -> std::io::Result<()> {
    match path::open_leaf_for_validation(path, "private file destination") {
        Ok(file) => owner::require_owned_by_eligible_principal(&file, "private file destination"),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_written_destination(path: &Path) -> std::io::Result<()> {
    let file = path::open_leaf_for_validation(path, "replaced private file")?;
    owner::require_owned_by_eligible_principal(&file, "replaced private file")
}

fn write_all_and_sync(mut file: File, bytes: &[u8]) -> std::io::Result<()> {
    file.write_all(bytes)?;
    file.sync_all()
}

fn invalid_input(message: &'static str) -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidData, message)
}

fn permission_denied(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(ErrorKind::PermissionDenied, message.into())
}

#[cfg(test)]
mod tests;
