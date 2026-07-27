mod common;

use agent_desktop_core::session::{
    GcOptions, SessionTraceMode, StartSessionOptions, gc, start_session, write_manifest,
};
use common::{ad_adapter_create_with_session, ad_adapter_destroy, with_isolated_home};
use std::ffi::CString;
use std::time::Duration;

/// Runs alone in its own process because private-file install state is
/// process-global: on Windows the first adapter construction installs
/// `WindowsPrivateFile`, whose atomic writes hold a process-lifetime temp
/// lease inside the written file's parent. If another adapter-creating test
/// ran first in this process, the session writes below would plant that
/// lease inside the session directory and the same-process gc removal would
/// fail by design. In the product, session writes and `session gc` never
/// share a process, so the isolation here models the real topology.
#[test]
fn session_scoped_adapter_holds_liveness_until_destroyed() {
    with_isolated_home(|| {
        let mut manifest = start_session(StartSessionOptions {
            name: None,
            trace: SessionTraceMode::Off,
            ..Default::default()
        })
        .unwrap();
        manifest.created_at = 0;
        write_manifest(&manifest).unwrap();
        let session = CString::new(manifest.id.as_str()).unwrap();
        let adapter = unsafe { ad_adapter_create_with_session(session.as_ptr()) };
        assert!(!adapter.is_null());

        let retained = gc(GcOptions {
            ended_only: false,
            older_than: Some(Duration::ZERO),
        })
        .unwrap();
        assert!(!retained.removed.contains(&manifest.id));

        unsafe { ad_adapter_destroy(adapter) };
        let removed = gc(GcOptions {
            ended_only: false,
            older_than: Some(Duration::ZERO),
        })
        .unwrap();
        assert!(removed.removed.contains(&manifest.id));
    });
}
