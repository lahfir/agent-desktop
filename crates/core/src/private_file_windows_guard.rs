use std::fs::File;
use std::path::{Component, Path, PathBuf};

use super::security::PrivateSecurity;

pub(super) struct AncestorGuards {
    handles: Vec<File>,
}

impl AncestorGuards {
    fn acquire(
        path: &Path,
        create: bool,
        security: Option<&PrivateSecurity>,
    ) -> std::io::Result<Self> {
        let absolute = super::path::normalized(path)?;
        let mut current = PathBuf::new();
        let mut handles = Vec::new();
        for component in absolute.components() {
            current.push(component);
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            let handle = match super::open_guarded_directory(&current) {
                Ok(handle) => handle,
                Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                    let security = security.ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::Unsupported,
                            "secure Windows directory creation requires a private descriptor",
                        )
                    })?;
                    super::create_private_directory(&current, security)?;
                    super::open_guarded_directory(&current)?
                }
                Err(error) => return Err(error),
            };
            super::validate_directory(&handle)?;
            handles.push(handle);
        }
        if handles.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Windows private path has no guardable directory",
            ));
        }
        Ok(Self { handles })
    }

    pub(super) fn leaf(&self) -> std::io::Result<&File> {
        self.handles.last().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Windows private path lost its ancestor guards",
            )
        })
    }
}

pub(super) fn with_ancestor_guards<T>(
    path: &Path,
    create: bool,
    security: Option<&PrivateSecurity>,
    operation: impl FnOnce(&AncestorGuards) -> std::io::Result<T>,
) -> std::io::Result<T> {
    let guards = AncestorGuards::acquire(path, create, security)?;
    operation(&guards)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifetime_contract(guards: &AncestorGuards) -> std::io::Result<&File> {
        guards.leaf()
    }

    #[test]
    fn operation_borrows_the_live_guard_chain() {
        let contract: for<'a> fn(&'a AncestorGuards) -> std::io::Result<&'a File> =
            lifetime_contract;
        let _ = contract;
    }
}
