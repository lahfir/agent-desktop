"""Private desktop requirements and shard ownership for the Linux live gate."""

TIERS = ("openbox", "mutter", "wayland")
WAYLAND_MODULES = (
    "input::clipboard::wayland::live_tests",
    "input::wayland::live_tests",
    "system::capture::wayland_portal_live_tests",
    "system::shell::live_wayland_tests",
)
MUTTER_MODULES = ("notifications::live_tests", "system::shell::outage_live_tests")
CURSOR_WAYLAND = "system::overlay::cursor_overlay::tests::live_wayland_cursor_refusal_reports_unrendered_without_an_x11_window"
CLI_WAYLAND = "system::overlay::cursor_overlay::cli_tests::live_wayland_cursor_cli_reports_unrendered_without_x11_fallback"
EXTERNAL_WAYLAND = (
    CURSOR_WAYLAND,
    CLI_WAYLAND,
    "system::process::protected::live_tests::a_native_wayland_application_is_cleared_by_its_tree_and_a_windowless_one_is_not",
)
BARE_CASES = (
    "input::physical::target_pointer::shell_live_tests::a_completed_wayland_click_with_refused_stop_is_delivered_once_and_unsafe_to_retry",
    "input::physical::target_pointer::shell_live_tests::a_missing_mutter_capability_refuses_before_session_creation",
)
OPENBOX_CASES = (
    "system::close::close_live_tests::close_live_the_private_sessions_own_processes_are_refused",
    "notifications::backend_live_tests::a_missing_daemon_keeps_its_detected_backend_refusal",
)
HELPER_PARENTS = {
    "system::window::window_op_live_tests::environment::window_op_live_child_stale_wayland":
        "system::window::window_op_live_tests::environment::window_op_live_a_stale_wayland_display_still_proceeds_on_x11",
    "system::window::window_op_live_tests::environment::no_display::window_op_live_child_no_display":
        "system::window::window_op_live_tests::environment::no_display::window_op_live_a_session_without_a_display_is_refused_before_any_write",
    "system::window::window_op_live_tests::environment::window_op_live_child_xwayland":
        "system::window::window_op_live_tests::environment::window_op_live_an_xwayland_window_is_operated_and_a_native_wayland_window_is_refused",
}
# Reviewed module groups whose fixtures never acquire the global interaction lease.
# Unknown modules and all unit tests stay on one writer, preserving that lock's contract.
PARALLEL_READS = {
    "tree::live::absence_live_tests", "tree::live::chromium_live_tests",
    "tree::live::hit_live_tests::cases", "tree::live::live_read_live_tests",
    "tree::live::navigate_live_tests", "tree::live::observe_live_tests", "tree::live::qt_live_tests",
    "tree::live::registry_toolkit_live_tests", "tree::live::resolve_live_tests",
    "tree::live::live_timeout",
    "tree::live::vocabulary_live_tests", "tree::live::walker_live_tests",
}
