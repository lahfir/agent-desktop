#[derive(Debug)]
pub(super) struct SessionScope {
    pub(super) id: String,
    pub(super) lease: Option<crate::session::SessionLivenessLease>,
}

impl Clone for SessionScope {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            lease: self.lease.clone(),
        }
    }
}
