use crate::{
    AdapterError, AppError, DeliverySemantics, ErrorCode, adapter::PlatformAdapter, cdp_endpoint,
    launch_options::LaunchOptions, launch_result::LaunchResult,
};
use serde_json::Value;

pub struct LaunchArgs {
    pub app: String,
    pub options: LaunchOptions,
}

pub fn execute(args: LaunchArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let LaunchArgs { app, mut options } = args;
    crate::wait_timeout_duration(options.timeout_ms)?;
    let deadline = if options.timeout_ms == 0 {
        crate::Deadline::standard()?
    } else {
        crate::Deadline::after(options.timeout_ms)?
    };
    let requested_cdp_port = options.cdp_port;
    if let Some(requested_port) = requested_cdp_port {
        reject_conflicting_cdp_switch(&options)?;
        reject_if_already_running(&app, adapter, deadline)?;
        let resolved_port = resolve_cdp_port(requested_port)?;
        options
            .args
            .push(format!("--remote-debugging-port={resolved_port}"));
        options.cdp_port = Some(resolved_port);
    }
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let mut launched = adapter.launch_app(&app, &options, &lease)?;
    if let Some(port) = options.cdp_port {
        launched.cdp = Some(verify_cdp_endpoint(port, deadline, &launched)?);
    }
    launched.suggestion = launch_suggestion(requested_cdp_port.is_some(), &launched);
    Ok(serde_json::to_value(launched)?)
}

/// Guidance the calling agent reads at the moment of use, not prose it has
/// to infer: a Chromium app launched without `--cdp` is nudged toward the
/// flag that opens the web-content door, and a verified endpoint is handed
/// off to a CDP client instead of left for the caller to rediscover.
fn launch_suggestion(cdp_requested: bool, launched: &LaunchResult) -> Option<String> {
    if launched.cdp.is_some() {
        return Some(
            "Drive the web contents with a CDP client: agent-browser connect <port> if \
             installed (or ask the user to install agent-browser). Native menus, dialogs, \
             windows, and screenshots stay with agent-desktop."
                .to_owned(),
        );
    }
    if !cdp_requested && launched.renderer.as_deref() == Some("chromium") {
        return Some(
            "Chromium app: for web-content work, quit with close-app and relaunch with --cdp, \
             then drive the web contents with a CDP client (agent-browser if installed). \
             Accessibility commands still cover everything, including native menus and dialogs."
                .to_owned(),
        );
    }
    None
}

/// Our own `--remote-debugging-port` switch is how `--cdp` works; a caller
/// who also passes one by hand would silently race it, so this is rejected
/// before launch rather than left to whichever switch Launch Services keeps.
fn reject_conflicting_cdp_switch(options: &LaunchOptions) -> Result<(), AppError> {
    let conflicts = options
        .args
        .iter()
        .any(|arg| arg.starts_with("--remote-debugging-port") || arg == "--remote-debugging-pipe");
    if !conflicts {
        return Ok(());
    }
    Err(AppError::Adapter(
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch already specifies a DevTools remote-debugging switch",
        )
        .with_details(serde_json::json!({ "kind": "cdp_switch_conflict" }))
        .with_suggestion("Pass either --cdp or your own --remote-debugging-port, not both.")
        .with_disposition(DeliverySemantics::not_delivered()),
    ))
}

/// The DevTools port only exists on the process that was launched with it;
/// attaching to an instance already running without it would report success
/// while the endpoint stays absent, so this is checked before launch instead
/// of discovered only once the endpoint fails to answer.
fn reject_if_already_running(
    id: &str,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<(), AppError> {
    let running = adapter.list_apps(deadline)?;
    let pids: Vec<_> = running
        .iter()
        .filter(|app| {
            app.name.eq_ignore_ascii_case(id)
                || app
                    .bundle_id
                    .as_deref()
                    .is_some_and(|bundle_id| bundle_id.eq_ignore_ascii_case(id))
        })
        .map(|app| app.pid)
        .collect();
    if pids.is_empty() {
        return Ok(());
    }
    Err(AppError::Adapter(
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The application is already running without the DevTools port",
        )
        .with_details(serde_json::json!({ "kind": "cdp_requires_fresh_launch", "pids": pids }))
        .with_suggestion(
            "The DevTools port only exists for a process launched with it. Quit the app with \
             close-app, then launch again with --cdp — or drive the running instance with \
             accessibility commands (snapshot/click).",
        )
        .with_disposition(DeliverySemantics::not_delivered()),
    ))
}

fn resolve_cdp_port(requested: u16) -> Result<u16, AppError> {
    if requested == 0 {
        return cdp_endpoint::pick_free_port();
    }
    if !cdp_endpoint::port_is_free(requested) {
        return Err(AppError::Adapter(
            AdapterError::new(
                ErrorCode::InvalidArgs,
                "Requested --cdp port is already in use",
            )
            .with_details(serde_json::json!({ "kind": "cdp_port_in_use", "port": requested }))
            .with_disposition(DeliverySemantics::not_delivered()),
        ));
    }
    Ok(requested)
}

/// Verifies the endpoint by observation instead of trusting the switch: the
/// app is left running either way, so a probe failure reports what exists
/// (pid, process instance) rather than merely that the flag was honored.
fn verify_cdp_endpoint(
    port: u16,
    deadline: crate::Deadline,
    launched: &LaunchResult,
) -> Result<cdp_endpoint::CdpEndpoint, AppError> {
    cdp_endpoint::probe(port, deadline).map_err(|_| {
        AppError::Adapter(
            AdapterError::new(
                ErrorCode::ActionFailed,
                "The launched application never opened its DevTools endpoint",
            )
            .with_details(serde_json::json!({
                "kind": "cdp_endpoint_unavailable",
                "pid": launched.pid,
                "port": port,
                "process_instance": launched.process_instance,
            }))
            .with_suggestion(
                "The app is running but never opened its DevTools endpoint — it may not be \
                 Chromium-based, or it strips debugging switches. Drive it with accessibility \
                 commands, or close-app it if it is unwanted.",
            )
            .with_disposition(DeliverySemantics::delivered_unverified()),
        )
    })
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "launch_cdp_tests.rs"]
mod cdp_tests;
