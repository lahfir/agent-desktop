use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;

/// Platform seam for the five private-file primitives.
///
/// Every method defaults to the portable behavior used when no platform
/// implementation is installed, so an implementation overrides only the
/// operations its filesystem semantics require.
pub trait PrivateFileOps: Send + Sync {
    /// Writes `bytes` to `path` as one atomic replacement, owning temporary
    /// creation, handle lifetime, and replace ordering end to end.
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        crate::private_file::write_atomic_portable(path, bytes)
    }

    /// Opens `path` for private appends; the returned handle must be lockable.
    fn open_private_append(&self, path: &Path) -> std::io::Result<File> {
        crate::private_file::open_private_append_portable(path)
    }

    /// Opens `path` read-write for locking, creating it when `create` is set.
    fn open_private_lock(&self, path: &Path, create: bool) -> std::io::Result<File> {
        crate::private_file::open_private_lock_portable(path, create)
    }

    /// Reads `path` fully, rejecting files larger than `max_bytes`.
    fn read_private_bounded(&self, path: &Path, max_bytes: u64) -> std::io::Result<Vec<u8>> {
        crate::private_file::read_private_bounded_portable(path, max_bytes)
    }

    /// Creates the directory chain at `path` and enforces that it is private.
    fn ensure_private(&self, path: &Path) -> std::io::Result<()> {
        crate::private_file_parent::ensure_private_portable(path)
    }
}

struct PortablePrivateFileOps;

impl PrivateFileOps for PortablePrivateFileOps {}

static INSTALLED_OPS: OnceLock<Box<dyn PrivateFileOps>> = OnceLock::new();

/// Installs the process-wide private-file operations.
///
/// The first installation wins; a later call leaves the installed operations
/// untouched and returns the rejected implementation.
pub fn install_private_file_ops(
    ops: Box<dyn PrivateFileOps>,
) -> Result<(), Box<dyn PrivateFileOps>> {
    INSTALLED_OPS.set(ops)
}

pub(crate) fn with_active_ops<R>(operate: impl FnOnce(&dyn PrivateFileOps) -> R) -> R {
    #[cfg(test)]
    if let Some(ops) = TEST_OPS_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return operate(ops.as_ref());
    }
    match INSTALLED_OPS.get() {
        Some(ops) => operate(ops.as_ref()),
        None => operate(&PortablePrivateFileOps),
    }
}

#[cfg(test)]
thread_local! {
    static TEST_OPS_OVERRIDE: std::cell::RefCell<Option<std::rc::Rc<dyn PrivateFileOps>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn with_test_ops_override<R>(
    ops: std::rc::Rc<dyn PrivateFileOps>,
    run: impl FnOnce() -> R,
) -> R {
    struct ClearOverrideOnDrop;
    impl Drop for ClearOverrideOnDrop {
        fn drop(&mut self) {
            TEST_OPS_OVERRIDE.with(|cell| cell.borrow_mut().take());
        }
    }
    TEST_OPS_OVERRIDE.with(|cell| *cell.borrow_mut() = Some(ops));
    let _clear_override = ClearOverrideOnDrop;
    run()
}

#[cfg(test)]
#[path = "private_file_ops_tests.rs"]
mod tests;
