#ifndef AGENT_DESKTOP_H
#define AGENT_DESKTOP_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/**
 * The major ABI version of this build of `libagent_desktop_ffi`.
 *
 * Version-bump rule: increment this constant (and update the header via
 * `scripts/update-ffi-header.sh`) whenever a breaking change is made to the
 * C ABI — a removed or incompatibly-changed `ad_*` symbol, or a layout
 * change to any `repr(C)` struct. Additive changes (new `ad_*` symbols, new
 * error codes) do **not** require a bump. Consumers must call `ad_init` with
 * the major they compiled against before making any adapter calls; a mismatch
 * returns `AD_RESULT_ERR_INVALID_ARGS` so they can refuse gracefully rather
 * than corrupt memory.
 */
#define AD_ABI_VERSION_MAJOR 1

/**
 * Maximum byte length (excluding the NUL terminator) accepted for any
 * foreign C string. Bounds both the terminator scan and the resulting
 * allocation, so a missing NUL or a hostile caller cannot walk arbitrary
 * memory into a `String`. Sized to roughly match the CLI's argv ceiling so
 * payload-bearing calls (clipboard-set, type) keep CLI parity rather than
 * being cut off at a ref-field-sized cap. Mirrored in the header as
 * `AD_MAX_STRING_BYTES`.
 */
#define AD_MAX_STRING_BYTES (1024 * 1024)

#define AD_ACTION_SIZE 96

#define AD_ACTION_RESULT_SIZE 40

#define AD_ACTION_STEP_SIZE 16

#define AD_DRAG_PARAMS_SIZE 48

#define AD_ELEMENT_STATE_SIZE 32

#define AD_REF_ENTRY_SIZE 192

/**
 * Per-field input caps enforced when converting an `AdRefEntry` at the C
 * boundary, sized from what real accessibility trees produce (a handful of
 * states/actions, double-digit path depth) with generous headroom. Mirrored
 * in the header so callers can validate before calling.
 */
#define AD_MAX_REF_STATES 64

#define AD_MAX_REF_ACTIONS 32

#define AD_MAX_REF_PATH_DEPTH 128

/**
 * Pinned size of `AdWaitArgs` on 64-bit targets. The compile-time
 * assert below and the `ad_wait_args_size()` runtime getter form the
 * 3-layer pin: Rust const assert, C `_Static_assert` in the header,
 * and the test in `c_abi_layout.rs`.
 */
#define AD_WAIT_ARGS_SIZE 112

enum AdResult
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_RESULT_OK = 0,
  AD_RESULT_ERR_PERM_DENIED = -1,
  AD_RESULT_ERR_ELEMENT_NOT_FOUND = -2,
  AD_RESULT_ERR_APP_NOT_FOUND = -3,
  AD_RESULT_ERR_ACTION_FAILED = -4,
  AD_RESULT_ERR_ACTION_NOT_SUPPORTED = -5,
  AD_RESULT_ERR_STALE_REF = -6,
  AD_RESULT_ERR_WINDOW_NOT_FOUND = -7,
  AD_RESULT_ERR_PLATFORM_NOT_SUPPORTED = -8,
  AD_RESULT_ERR_TIMEOUT = -9,
  AD_RESULT_ERR_INVALID_ARGS = -10,
  AD_RESULT_ERR_NOTIFICATION_NOT_FOUND = -11,
  AD_RESULT_ERR_INTERNAL = -12,
  AD_RESULT_ERR_SNAPSHOT_NOT_FOUND = -13,
  AD_RESULT_ERR_POLICY_DENIED = -14,
  AD_RESULT_ERR_AMBIGUOUS_TARGET = -15,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdResult AdResult;
#else
typedef int32_t AdResult;
#endif // __STDC_VERSION__ >= 202311L

enum AdImageFormat
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_IMAGE_FORMAT_PNG = 0,
  AD_IMAGE_FORMAT_JPG = 1,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdImageFormat AdImageFormat;
#else
typedef int32_t AdImageFormat;
#endif // __STDC_VERSION__ >= 202311L

enum AdActionKind
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_ACTION_KIND_CLICK = 0,
  AD_ACTION_KIND_DOUBLE_CLICK = 1,
  AD_ACTION_KIND_RIGHT_CLICK = 2,
  AD_ACTION_KIND_TRIPLE_CLICK = 3,
  AD_ACTION_KIND_SET_VALUE = 4,
  AD_ACTION_KIND_SET_FOCUS = 5,
  AD_ACTION_KIND_EXPAND = 6,
  AD_ACTION_KIND_COLLAPSE = 7,
  AD_ACTION_KIND_SELECT = 8,
  AD_ACTION_KIND_TOGGLE = 9,
  AD_ACTION_KIND_CHECK = 10,
  AD_ACTION_KIND_UNCHECK = 11,
  AD_ACTION_KIND_SCROLL = 12,
  AD_ACTION_KIND_SCROLL_TO = 13,
  AD_ACTION_KIND_PRESS_KEY = 14,
  AD_ACTION_KIND_KEY_DOWN = 15,
  AD_ACTION_KIND_KEY_UP = 16,
  AD_ACTION_KIND_TYPE_TEXT = 17,
  AD_ACTION_KIND_CLEAR = 18,
  AD_ACTION_KIND_HOVER = 19,
  AD_ACTION_KIND_DRAG = 20,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdActionKind AdActionKind;
#else
typedef int32_t AdActionKind;
#endif // __STDC_VERSION__ >= 202311L

enum AdDirection
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_DIRECTION_UP = 0,
  AD_DIRECTION_DOWN = 1,
  AD_DIRECTION_LEFT = 2,
  AD_DIRECTION_RIGHT = 3,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdDirection AdDirection;
#else
typedef int32_t AdDirection;
#endif // __STDC_VERSION__ >= 202311L

enum AdModifier
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_MODIFIER_CMD = 0,
  AD_MODIFIER_CTRL = 1,
  AD_MODIFIER_ALT = 2,
  AD_MODIFIER_SHIFT = 3,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdModifier AdModifier;
#else
typedef int32_t AdModifier;
#endif // __STDC_VERSION__ >= 202311L

enum AdMouseButton
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_MOUSE_BUTTON_LEFT = 0,
  AD_MOUSE_BUTTON_RIGHT = 1,
  AD_MOUSE_BUTTON_MIDDLE = 2,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdMouseButton AdMouseButton;
#else
typedef int32_t AdMouseButton;
#endif // __STDC_VERSION__ >= 202311L

enum AdMouseEventKind
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_MOUSE_EVENT_KIND_MOVE = 0,
  AD_MOUSE_EVENT_KIND_DOWN = 1,
  AD_MOUSE_EVENT_KIND_UP = 2,
  AD_MOUSE_EVENT_KIND_CLICK = 3,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdMouseEventKind AdMouseEventKind;
#else
typedef int32_t AdMouseEventKind;
#endif // __STDC_VERSION__ >= 202311L

enum AdPolicyKind
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_POLICY_KIND_HEADLESS = 0,
  AD_POLICY_KIND_FOCUS_FALLBACK = 1,
  AD_POLICY_KIND_HEADED = 2,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdPolicyKind AdPolicyKind;
#else
typedef int32_t AdPolicyKind;
#endif // __STDC_VERSION__ >= 202311L

enum AdScreenshotKind
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_SCREENSHOT_KIND_SCREEN = 0,
  AD_SCREENSHOT_KIND_WINDOW = 1,
  AD_SCREENSHOT_KIND_FULL_SCREEN = 2,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdScreenshotKind AdScreenshotKind;
#else
typedef int32_t AdScreenshotKind;
#endif // __STDC_VERSION__ >= 202311L

enum AdSnapshotSurface
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_SNAPSHOT_SURFACE_WINDOW = 0,
  AD_SNAPSHOT_SURFACE_FOCUSED = 1,
  AD_SNAPSHOT_SURFACE_MENU = 2,
  AD_SNAPSHOT_SURFACE_MENUBAR = 3,
  AD_SNAPSHOT_SURFACE_SHEET = 4,
  AD_SNAPSHOT_SURFACE_POPOVER = 5,
  AD_SNAPSHOT_SURFACE_ALERT = 6,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdSnapshotSurface AdSnapshotSurface;
#else
typedef int32_t AdSnapshotSurface;
#endif // __STDC_VERSION__ >= 202311L

enum AdWindowOpKind
#if __STDC_VERSION__ >= 202311L
  : int32_t
#endif // __STDC_VERSION__ >= 202311L
 {
  AD_WINDOW_OP_KIND_RESIZE = 0,
  AD_WINDOW_OP_KIND_MOVE = 1,
  AD_WINDOW_OP_KIND_MINIMIZE = 2,
  AD_WINDOW_OP_KIND_MAXIMIZE = 3,
  AD_WINDOW_OP_KIND_RESTORE = 4,
};
#if __STDC_VERSION__ >= 202311L
typedef enum AdWindowOpKind AdWindowOpKind;
#else
typedef int32_t AdWindowOpKind;
#endif // __STDC_VERSION__ >= 202311L

typedef struct AdAdapter AdAdapter;

/**
 * Opaque list handle emitted by `ad_list_apps`. See
 * [`crate::types::window_list::AdWindowList`] for the pattern.
 */
typedef struct AdAppList AdAppList;

/**
 * Opaque image-buffer handle returned by `ad_screenshot`. The backing
 * byte buffer and its length live inside the Rust-owned struct — a
 * consumer cannot accidentally desynchronize the pair and trigger a
 * heap-corruption double-free. Walk it through `ad_image_buffer_*`
 * accessors and free it with `ad_image_buffer_free`.
 */
typedef struct AdImageBuffer AdImageBuffer;

/**
 * Opaque notification list returned by `ad_list_notifications`.
 */
typedef struct AdNotificationList AdNotificationList;

/**
 * Opaque list handle emitted by `ad_list_surfaces`. See
 * [`crate::types::window_list::AdWindowList`] for the pattern.
 */
typedef struct AdSurfaceList AdSurfaceList;

/**
 * Opaque list handle emitted by `ad_list_windows`.
 *
 * The struct intentionally has no `#[repr(C)]` so cbindgen emits a
 * forward declaration only (`typedef struct AdWindowList AdWindowList;`).
 * Consumers cannot read the backing pointer or length and cannot
 * construct a count mismatch — they walk the list through
 * `ad_window_list_count`, `ad_window_list_get`, and free it with
 * `ad_window_list_free`.
 */
typedef struct AdWindowList AdWindowList;

typedef struct AdNativeHandle {
  const void *ptr;
} AdNativeHandle;

/**
 * Scroll parameters embedded in `AdAction` when `kind == SCROLL`.
 *
 * `direction` is stored as `int32_t` for the same boundary-safety
 * reason `AdAction.kind` is. Valid values are the discriminants of
 * `AdDirection`.
 */
typedef struct AdScrollParams {
  int32_t direction;
  uint32_t amount;
} AdScrollParams;

/**
 * Key combination: a named key plus optional modifier list.
 *
 * `modifiers` points to an array of `int32_t` values (not a typed Rust
 * enum array) so the C boundary cannot be tricked into writing an
 * out-of-range discriminant into a Rust enum slot. Each entry is
 * validated against `AdModifier` before use; an invalid discriminant
 * returns `AD_RESULT_ERR_INVALID_ARGS`.
 */
typedef struct AdKeyCombo {
  const char *key;
  const int32_t *modifiers;
  uint32_t modifier_count;
} AdKeyCombo;

typedef struct AdPoint {
  double x;
  double y;
} AdPoint;

/**
 * Caller-allocated drag parameters. Callers must zero-initialize the whole
 * struct before setting fields so unset numeric fields read as the `0`
 * adapter-default sentinel rather than stack garbage. Verify layout against
 * `AD_DRAG_PARAMS_SIZE` / `ad_drag_params_size()` when binding from a language
 * whose struct layout may diverge.
 */
typedef struct AdDragParams {
  struct AdPoint from;
  struct AdPoint to;
  uint64_t duration_ms;
  uint64_t drop_delay_ms;
} AdDragParams;

/**
 * Action dispatched by `ad_execute_action`.
 *
 * `kind` is stored as `int32_t` so a buggy or malicious C caller
 * cannot write an out-of-range discriminant into a Rust enum slot —
 * an out-of-range value is rejected with
 * `AD_RESULT_ERR_INVALID_ARGS` at the boundary. Valid values are the
 * discriminants of `AdActionKind`.
 *
 * `AdDragParams` is embedded by value, so any growth there grows this
 * struct too. Callers must zero-initialize the whole struct and verify
 * layout against `AD_ACTION_SIZE` / `ad_action_size()` when binding from
 * a language whose struct layout may diverge — an under-allocated action
 * makes the library read past the caller's buffer.
 */
typedef struct AdAction {
  int32_t kind;
  const char *text;
  struct AdScrollParams scroll;
  struct AdKeyCombo key;
  struct AdDragParams drag;
} AdAction;

typedef struct AdElementState {
  const char *role;
  char **states;
  uint32_t state_count;
  const char *value;
} AdElementState;

typedef struct AdActionStep {
  const char *label;
  const char *outcome;
} AdActionStep;

typedef struct AdActionResult {
  const char *action;
  const char *ref_id;
  struct AdElementState *post_state;
  struct AdActionStep *steps;
  uint32_t step_count;
} AdActionResult;

typedef struct AdRect {
  double x;
  double y;
  double width;
  double height;
} AdRect;

typedef struct AdRefEntry {
  int32_t pid;
  const char *role;
  const char *name;
  const char *value;
  const char *description;
  const char *const *states;
  size_t state_count;
  const char *const *available_actions;
  size_t available_action_count;
  struct AdRect bounds;
  bool has_bounds;
  uint64_t bounds_hash;
  bool has_bounds_hash;
  const char *source_app;
  const char *source_window_id;
  const char *source_window_title;
  int32_t source_surface;
  const char *root_ref;
  bool path_is_absolute;
  const uint32_t *path;
  size_t path_count;
} AdRefEntry;

typedef struct AdWindowInfo {
  const char *id;
  const char *title;
  const char *app_name;
  int32_t pid;
  struct AdRect bounds;
  bool has_bounds;
  bool is_focused;
} AdWindowInfo;

typedef struct AdAppInfo {
  const char *name;
  int32_t pid;
  const char *bundle_id;
} AdAppInfo;

/**
 * Arguments for `ad_wait`, mirroring `core::commands::wait::WaitArgs`.
 *
 * Fields map as follows:
 * - `Option<u64>` → `u64` value + `bool has_*` sentinel (ms, count).
 * - `Option<String>` → nullable `*const c_char` (null = absent).
 * - `bool` → `bool`.
 *
 * Callers must zero-initialize before use and verify layout via
 * `AD_WAIT_ARGS_SIZE` / `ad_wait_args_size()`.
 */
typedef struct AdWaitArgs {
  /**
   * Milliseconds to sleep (WaitMode::ms).
   */
  uint64_t ms;
  bool has_ms;
  /**
   * Element ref id to wait for (WaitMode::element).
   */
  const char *element;
  /**
   * Window title to wait for (WaitMode::window).
   */
  const char *window;
  /**
   * Text to wait for (WaitMode::text / WaitMode::notification text).
   */
  const char *text;
  /**
   * Wait for menu to open (true) or close (false via menu_closed).
   */
  bool menu;
  /**
   * Wait for menu to close.
   */
  bool menu_closed;
  /**
   * Wait for a notification.
   */
  bool notification;
  /**
   * Snapshot id for element predicate (WaitPredicateArgs::snapshot_id).
   */
  const char *snapshot_id;
  /**
   * Predicate kind string (WaitPredicateArgs::predicate).
   */
  const char *predicate;
  /**
   * Expected value for value-predicate (WaitPredicateArgs::value).
   */
  const char *value;
  /**
   * Action name for actionability-predicate (WaitPredicateArgs::action).
   */
  const char *action;
  /**
   * Expected match count for text waits (WaitPredicateArgs::count).
   */
  size_t count;
  bool has_count;
  /**
   * Timeout in milliseconds.
   */
  uint64_t timeout_ms;
  /**
   * App name filter (null = any). Maps to WaitArgs::app.
   */
  const char *app;
} AdWaitArgs;

/**
 * Mouse event dispatched by `ad_mouse_event`.
 *
 * `kind` and `button` are stored as `int32_t` for the same reason
 * `AdAction.kind` is — foreign callers cannot place invalid
 * discriminants into Rust enum slots. Valid values are the
 * discriminants of `AdMouseEventKind` and `AdMouseButton`.
 */
typedef struct AdMouseEvent {
  int32_t kind;
  struct AdPoint point;
  int32_t button;
  uint32_t click_count;
} AdMouseEvent;

typedef struct AdNotificationFilter {
  const char *app;
  const char *text;
  uint32_t limit;
  bool has_limit;
} AdNotificationFilter;

typedef struct AdNotificationInfo {
  uint32_t index;
  const char *app_name;
  const char *title;
  const char *body;
  char **actions;
  uint32_t action_count;
} AdNotificationInfo;

typedef struct AdFindQuery {
  const char *role;
  const char *name_substring;
  const char *value_substring;
} AdFindQuery;

/**
 * Screenshot target for `ad_screenshot`.
 *
 * `kind` is stored as `int32_t` to keep the enum-discriminant check
 * at the boundary. Valid values are the discriminants of
 * `AdScreenshotKind`. `screen_index` is only consulted when kind is
 * `SCREEN`; `pid` only when kind is `WINDOW`.
 */
typedef struct AdScreenshotTarget {
  int32_t kind;
  uint64_t screen_index;
  int32_t pid;
} AdScreenshotTarget;

typedef struct AdSurfaceInfo {
  const char *kind;
  const char *title;
  int64_t item_count;
} AdSurfaceInfo;

typedef struct AdNode {
  const char *ref_id;
  const char *role;
  const char *name;
  const char *value;
  const char *description;
  const char *hint;
  char **states;
  uint32_t state_count;
  struct AdRect bounds;
  bool has_bounds;
  int32_t parent_index;
  uint32_t child_start;
  uint32_t child_count;
} AdNode;

typedef struct AdNodeTree {
  struct AdNode *nodes;
  uint32_t count;
} AdNodeTree;

/**
 * Options for `ad_get_tree`.
 *
 * `surface` is stored as `int32_t` so foreign callers cannot write
 * an invalid discriminant into a Rust enum slot. Valid values are the
 * discriminants of `AdSnapshotSurface`; out-of-range values return
 * `AD_RESULT_ERR_INVALID_ARGS`.
 */
typedef struct AdTreeOptions {
  uint8_t max_depth;
  bool include_bounds;
  bool interactive_only;
  bool compact;
  int32_t surface;
} AdTreeOptions;

/**
 * Window-manager operation dispatched by `ad_window_op`.
 *
 * `kind` is stored as `int32_t` to keep the enum-discriminant check at
 * the boundary — out-of-range values return
 * `AD_RESULT_ERR_INVALID_ARGS`. Valid values are the discriminants of
 * `AdWindowOpKind`. `width`/`height`/`x`/`y` are only consulted for
 * the variants that use them.
 */
typedef struct AdWindowOp {
  int32_t kind;
  double width;
  double height;
  double x;
  double y;
} AdWindowOp;

/**
 * Returns the packed ABI major version of this dylib build.
 *
 * A consumer should compare this to `AD_ABI_VERSION_MAJOR` from the header it
 * compiled against. If they differ, call nothing further — the ABI is
 * incompatible.
 */
uint32_t ad_abi_version(void);

/**
 * Validates that the consumer's expected ABI major matches this dylib.
 *
 * Call once after `dlopen` / `LoadLibrary`, before any adapter call.
 * Returns `AD_RESULT_OK` when `expected_major == AD_ABI_VERSION_MAJOR`.
 * Returns `AD_RESULT_ERR_INVALID_ARGS` with a diagnostic last-error when the
 * version does not match, so the consumer can refuse to proceed rather than
 * crash with an incompatible ABI.
 */
AdResult ad_init(uint32_t expected_major);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `handle` must be a non-null pointer to a valid `AdNativeHandle` produced by
 * the same live adapter. Free the handle before destroying that adapter.
 * `action` must be a non-null pointer to a valid `AdAction`.
 * `out` must be a non-null pointer to an `AdActionResult` to write the result into.
 */
AdResult ad_execute_action(const struct AdAdapter *adapter,
                           const struct AdNativeHandle *handle,
                           const struct AdAction *action,
                           struct AdActionResult *out);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `handle` must be a non-null pointer to a valid `AdNativeHandle` produced by
 * the same live adapter. Free the handle before destroying that adapter.
 * `action` must be a non-null pointer to a valid `AdAction`.
 * `out` must be a non-null pointer to an `AdActionResult` to write the result into.
 */
AdResult ad_execute_action_with_policy(const struct AdAdapter *adapter,
                                       const struct AdNativeHandle *handle,
                                       const struct AdAction *action,
                                       int32_t policy,
                                       struct AdActionResult *out);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `entry` must be a non-null pointer to a valid `AdRefEntry`.
 * `action` must be a non-null pointer to a valid `AdAction`.
 * `out` must be a non-null pointer to an `AdActionResult` to write the result into.
 */
AdResult ad_execute_ref_action_with_policy(const struct AdAdapter *adapter,
                                           const struct AdRefEntry *entry,
                                           const struct AdAction *action,
                                           int32_t policy,
                                           struct AdActionResult *out);

/**
 * Releases a handle previously returned by `ad_resolve_element` and
 * zeroes the caller's struct so accidentally calling this twice is
 * a deterministic no-op instead of a double-free on the underlying
 * `CFRelease`.
 *
 * On macOS this calls `CFRelease` on the underlying `AXUIElementRef`,
 * balancing the `CFRetain` that happened during `ad_resolve_element`.
 * On Windows/Linux the call is a no-op that returns `AD_RESULT_OK`
 * (platform adapters inherit the default `not_supported` impl; the
 * FFI surface translates it so callers apply the same release
 * pattern everywhere).
 *
 * Ownership contract: the FFI owns the handle from the moment
 * `ad_resolve_element` writes `ptr`. Copying the struct after that
 * point and calling `ad_free_handle` on either copy is undefined —
 * there is no way for the library to detect forged non-null pointers.
 * Callers that legitimately need a "copy" should re-resolve.
 *
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`
 * and must be the same live adapter that produced `handle`.
 * `handle` must be null or a `*mut AdNativeHandle` previously
 * populated by `ad_resolve_element`. On return `(*handle).ptr` is
 * `NULL` so a double-call is a no-op instead of a double-free.
 */
AdResult ad_free_handle(const struct AdAdapter *adapter, struct AdNativeHandle *handle);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `entry` must be a non-null pointer to a valid `AdRefEntry`.
 * `out` must be a non-null pointer to an `AdNativeHandle` to write the result into.
 */
AdResult ad_resolve_element(const struct AdAdapter *adapter,
                            const struct AdRefEntry *entry,
                            struct AdNativeHandle *out);

/**
 * # Safety
 *
 * `result` must be null or a pointer to an `AdActionResult` previously written
 * by `ad_execute_action`, `ad_execute_action_with_policy`,
 * `ad_execute_ref_action_with_policy`, or `ad_notification_action`. This frees
 * `post_state`, `steps`, and all nested strings. After this call all pointers
 * inside the struct are invalid.
 */
void ad_free_action_result(struct AdActionResult *result);

/**
 * Builds a platform adapter for the current OS and returns an opaque
 * handle. Returns null on allocation failure or if a Rust panic is
 * caught at the FFI boundary (inspect `ad_last_error_*` for details).
 *
 * The returned pointer is owned by the caller and must be released with
 * `ad_adapter_destroy`. Creating and destroying adapters is cheap; the
 * common pattern is one adapter per process lifetime.
 */
struct AdAdapter *ad_adapter_create(void);

/**
 * Builds a session-scoped platform adapter. `session` may be:
 * - null: equivalent to `ad_adapter_create()` (no session).
 * - a valid session id (1-64 ASCII alphanumeric / `-` / `_` chars): associates
 *   the adapter with that session for refmap persistence.
 * - empty, too long, containing invalid characters, or invalid UTF-8: sets
 *   `ErrInvalidArgs` in the last-error slot and returns null; no adapter is
 *   allocated.
 *
 * The returned pointer must be released with `ad_adapter_destroy`.
 *
 * # Safety
 *
 * `session` must be null or point to readable memory that is NUL-terminated
 * within `AD_MAX_STRING_BYTES + 1` bytes.
 */
struct AdAdapter *ad_adapter_create_with_session(const char *session);

/**
 * # Safety
 *
 * `adapter` must be a pointer returned by `ad_adapter_create`, or null.
 * After this call the pointer is invalid and must not be used.
 */
void ad_adapter_destroy(struct AdAdapter *adapter);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create` that
 * has not yet been destroyed.
 */
AdResult ad_check_permissions(const struct AdAdapter *adapter);

/**
 * Closes the application identified by `id` (bundle id on macOS,
 * executable path on other platforms). `force = true` skips the
 * graceful-shutdown path, terminates matching app processes, and escalates
 * survivors when the platform supports it. Session-critical
 * processes (loginwindow, WindowServer, Dock, Finder, launchd) are
 * refused with `AD_RESULT_ERR_INVALID_ARGS` — the protected-process
 * guard is enforced inside the adapter, so FFI and CLI behave
 * identically.
 *
 * # Safety
 * `adapter` must be non-null. `id` must be a non-null UTF-8 C string.
 */
AdResult ad_close_app(const struct AdAdapter *adapter, const char *id, bool force);

/**
 * Launches the application identified by `id` (bundle id on macOS,
 * executable path on other platforms) and, on success, writes the
 * first window that becomes available into `*out`. Waits up to
 * `timeout_ms` for the window to appear; zero means "no wait".
 *
 * The returned `AdWindowInfo` owns heap-allocated interior strings that
 * must be released with `ad_release_window_fields` once done. On error
 * the out-param is zero-initialized, so calling the release fn on it
 * is a safe no-op.
 *
 * # Safety
 * `adapter` must be non-null. `id` must be a non-null UTF-8 C string.
 * `out` must be a non-null writable `*mut AdWindowInfo`.
 */
AdResult ad_launch_app(const struct AdAdapter *adapter,
                       const char *id,
                       uint64_t timeout_ms,
                       struct AdWindowInfo *out);

/**
 * # Safety
 * `adapter` must be a valid pointer from `ad_adapter_create`.
 * `out` must be a valid writable `*mut *mut AdAppList`.
 * On success, `*out` is a newly-allocated opaque list freed with
 * `ad_app_list_free`. On error, `*out` is null and last-error is set.
 */
AdResult ad_list_apps(const struct AdAdapter *adapter, struct AdAppList **out);

/**
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_apps`.
 */
uint32_t ad_app_list_count(const struct AdAppList *list);

/**
 * Returns a borrowed pointer into the list; valid until the list is freed.
 * Out-of-range `index` returns null.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_apps`.
 */
const struct AdAppInfo *ad_app_list_get(const struct AdAppList *list, uint32_t index);

/**
 * Frees the list and every `AdAppInfo` it owns, including the interior
 * C-strings.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_apps`.
 */
void ad_app_list_free(struct AdAppList *list);

/**
 * Drives a ref action (`@e5`, action) through the full strict-resolution
 * ladder: `RefStore` load → `RefMap` lookup (→ `STALE_REF` on missing) →
 * `resolve_element_strict` (→ `STALE_REF`/`AMBIGUOUS_TARGET`) → live
 * actionability preflight → dispatch → handle release.
 *
 * Policy follows CLI parity (KTD6): `TypeText` actions default to
 * `focus_fallback`; every other action defaults to `headless`. An explicit
 * `policy` discriminant may *elevate* to headed but must not downgrade an
 * action below its CLI base.
 *
 * `ref_id` tri-state: null → `ErrInvalidArgs`; non-null invalid UTF-8 →
 * `ErrInvalidArgs`; valid UTF-8 but bad `@e{N}` format → `ErrInvalidArgs`.
 *
 * `policy` is an `AdPolicyKind` discriminant (0=Headless, 1=FocusFallback,
 * 2=Headed). An out-of-range value returns `ErrInvalidArgs`. `Headless (0)`
 * accepts the action's own CLI base (so `TypeText` still uses
 * `focus_fallback`). `Headed (2)` opts in to cursor-based fallbacks.
 *
 * On success `*out` is set to a NUL-terminated JSON envelope (command
 * `"execute_by_ref"`); free with `ad_free_string`. On guard or decode
 * failure (invalid args before the command runs) `*out` remains null.
 * On a command-level error (STALE_REF, AMBIGUOUS_TARGET, etc.) `*out`
 * holds the error JSON envelope and must still be freed with
 * `ad_free_string`. The last-error slot is populated on all failures.
 *
 * # Safety
 *
 * `adapter` must be a non-null pointer from `ad_adapter_create[_with_session]`.
 * `ref_id` must be null or NUL-terminated within `AD_MAX_STRING_BYTES + 1`
 * bytes. `action` must be a non-null pointer to a valid `AdAction`.
 * `out` must be a non-null writable pointer. All pointers must remain valid
 * for the duration of the call. Must be called from the main thread on macOS.
 */
AdResult ad_execute_by_ref(const struct AdAdapter *adapter,
                           const char *ref_id,
                           const struct AdAction *action,
                           int32_t policy,
                           char **out);

/**
 * Takes a full CLI-format snapshot of the target application window,
 * allocates `@e` refs for all interactive elements, persists the refmap
 * to disk, and writes the JSON envelope into `*out`.
 *
 * The JSON shape matches `agent-desktop snapshot`:
 * `{"version":"2.0","ok":true,"command":"snapshot","data":{"app":"...","window":{...},"ref_count":N,"snapshot_id":"...","tree":{...}}}`.
 *
 * **`*out` ownership and error behaviour:**
 * - On success (`AD_RESULT_OK`): `*out` is a heap-allocated JSON string with `"ok":true`.
 *   Caller must free it with `ad_free_string`.
 * - On a command-level error (e.g. app not found, snapshot failure): `*out` is a
 *   heap-allocated JSON string with `"ok":false` and an `"error"` payload. Caller
 *   must still free it with `ad_free_string`. The last-error slot is also set.
 * - On an argument or infrastructure error (null adapter, off-main-thread, invalid
 *   UTF-8, bad surface discriminant, context failure): `*out` is set to null and no
 *   allocation is made. Only the last-error slot is set.
 *
 * `app` is tri-state:
 * - null — snapshot the currently focused window (same as running the command with no `--app`).
 * - valid UTF-8 string — snapshot the named application's focused window.
 * - non-null but invalid UTF-8 or exceeding `AD_MAX_STRING_BYTES` — returns `ErrInvalidArgs`.
 *
 * `surface` is an `AdSnapshotSurface` discriminant (0 = Window, 1 = Focused, …).
 * An out-of-range value returns `ErrInvalidArgs`.
 *
 * Skeleton mode and `--root` drill-down are not exposed here; they are a
 * fast-follow to this entrypoint.
 *
 * # Safety
 *
 * `adapter` must be a non-null pointer from `ad_adapter_create` or
 * `ad_adapter_create_with_session`. `out` must be a non-null writable
 * `*mut *mut c_char`. `app` must be null or a NUL-terminated string within
 * `AD_MAX_STRING_BYTES + 1` bytes. All pointers must remain valid for the
 * duration of the call. `adapter` must be used from the main thread on macOS.
 */
AdResult ad_snapshot(const struct AdAdapter *adapter,
                     const char *app,
                     int32_t surface,
                     uint8_t max_depth,
                     bool interactive_only,
                     bool compact,
                     char **out);

/**
 * Returns the adapter's current health and permission state as a JSON
 * envelope matching the `agent-desktop status` CLI output.
 *
 * `ad_status` does not query the accessibility tree; it reads the
 * permission report and ref-store metadata only, so it is safe to call
 * from any thread (unlike tree-traversal commands that require the
 * macOS main thread). On success `*out` is a NUL-terminated,
 * heap-allocated JSON string freed with `ad_free_string`. On error
 * `*out` is left null and the last-error slot is populated.
 *
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`
 * that has not been destroyed. `out` must be a non-null writable
 * `*mut *mut c_char`.
 */
AdResult ad_status(const struct AdAdapter *adapter, char **out);

/**
 * Returns the `agent-desktop` version envelope as an owned JSON C string.
 *
 * The returned string has the same `{version, ok, command, data}` shape
 * as `agent-desktop version` on the CLI. Free it with `ad_free_string`.
 *
 * On success `*out` points to the envelope JSON.
 * On error `*out` is null and the last-error slot is populated.
 *
 * # Safety
 * `out` must be a non-null writable `*mut *mut c_char`.
 */
AdResult ad_version(char **out);

/**
 * Runs `wait` with the given args, blocking the calling thread until the
 * condition is met or `timeout_ms` elapses.
 *
 * On success `*out` is set to a freshly allocated JSON string containing the
 * CLI-format wait envelope (`{version, ok, command, data}`). The caller must
 * release the string with `ad_free_string(*out)`.
 *
 * On failure `*out` is zeroed, the last-error slot is set, and a negative
 * `AdResult` code is returned.
 *
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create` that
 * has not been destroyed. `args` must be non-null and point to a valid
 * zero-initialized `AdWaitArgs`. `out` must be non-null and point to a
 * writable `*mut c_char`.
 *
 * All `*const c_char` fields inside `AdWaitArgs` must be null or point to
 * readable, NUL-terminated memory within `AD_MAX_STRING_BYTES + 1` bytes.
 */
AdResult ad_wait(const struct AdAdapter *adapter, const struct AdWaitArgs *args, char **out);

/**
 * Last-error lifetime — errno-style.
 *
 * The pointer returned by `ad_last_error_message`,
 * `ad_last_error_suggestion`, and `ad_last_error_platform_detail`
 * remains valid across any number of subsequent **successful** FFI
 * calls on the same thread. Only the next FFI call that itself **fails**
 * (returns a non-`AD_RESULT_OK` code) invalidates the previous pointers.
 *
 * Consumers can therefore read an error once, cache the pointer, and
 * keep reading it back across follow-up work that clears or re-fetches
 * state before handing control to the user.
 *
 * This matches the POSIX `errno` / `strerror` contract and is scoped
 * per-thread via thread-local storage — Thread A's last-error never
 * leaks to Thread B.
 * Returns the `AdResult` code of the last error on the calling thread,
 * or `AD_RESULT_OK` if no error has been recorded.
 */
AdResult ad_last_error_code(void);

/**
 * Returns a borrowed C string describing the last error, or null if no
 * error has been recorded on the calling thread. The pointer remains
 * valid across any number of subsequent *successful* FFI calls; only
 * the next failing call overwrites it.
 */
const char *ad_last_error_message(void);

/**
 * Returns a borrowed C string with a human-readable suggestion for how
 * to recover from the last error, or null if the adapter didn't emit
 * one. Same lifetime rules as `ad_last_error_message`.
 */
const char *ad_last_error_suggestion(void);

/**
 * Returns a borrowed C string carrying a platform-specific diagnostic
 * for the last error (AX error codes, COM HRESULTs, AT-SPI messages,
 * etc.), or null if the adapter didn't supply one. Same lifetime rules
 * as `ad_last_error_message`.
 */
const char *ad_last_error_platform_detail(void);

/**
 * Returns a borrowed JSON string carrying structured details for the last
 * error, or null if the adapter didn't supply any. Same lifetime rules as
 * `ad_last_error_message`.
 */
const char *ad_last_error_details(void);

/**
 * Reads the current clipboard text and writes an owned C string into
 * `*out`. The caller must free the returned pointer with
 * `ad_free_string`. On error `*out` is left null.
 *
 * # Safety
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `out` must be a non-null writable `*mut *mut c_char`.
 */
AdResult ad_get_clipboard(const struct AdAdapter *adapter, char **out);

/**
 * Writes UTF-8 `text` to the clipboard. Null or non-UTF-8 input returns
 * `AD_RESULT_ERR_INVALID_ARGS` with a diagnostic last-error.
 *
 * # Safety
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `text` must be a non-null, NUL-terminated UTF-8 C string.
 */
AdResult ad_set_clipboard(const struct AdAdapter *adapter, const char *text);

/**
 * Clears the clipboard.
 *
 * # Safety
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 */
AdResult ad_clear_clipboard(const struct AdAdapter *adapter);

/**
 * Frees a C string previously returned by `ad_get_clipboard` or any
 * other FFI call documented as allocating a C string for the caller.
 * Null-tolerant — safe to call on `NULL`. Double-free is undefined.
 *
 * # Safety
 * `s` must be null or a pointer previously handed out by this crate.
 * After this call the pointer is invalid and must not be used.
 */
void ad_free_string(char *s);

/**
 * Synthesizes an explicit physical mouse drag from `params.from` to
 * `params.to`. When `params.duration_ms` is zero the drag is instantaneous;
 * a non-zero value asks the platform adapter to interpolate. Callers that
 * need headless policy enforcement should use ref actions with policy.
 *
 * # Safety
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `params` must be a non-null pointer to a valid `AdDragParams`.
 */
AdResult ad_drag(const struct AdAdapter *adapter, const struct AdDragParams *params);

/**
 * Dispatches an explicit physical mouse event (move / down / up / click)
 * at the given screen point. Click count is only consulted when `event.kind`
 * is `CLICK` (e.g., `click_count == 2` for a double-click). Callers that
 * need headless policy enforcement should use ref actions with policy.
 *
 * # Safety
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `event` must be a non-null pointer to a valid `AdMouseEvent`.
 */
AdResult ad_mouse_event(const struct AdAdapter *adapter, const struct AdMouseEvent *event);

/**
 * Registers a callback to receive `tracing` events, or unregisters the
 * current callback when `cb` is `NULL`.
 *
 * The subscriber layer is installed exactly once (the first time a non-null
 * callback is set). Subsequent calls only swap the stored pointer, never
 * re-install the layer.
 *
 * The callback receives:
 * - `level` — 1 (ERROR) … 5 (TRACE)
 * - `msg` — a NUL-terminated JSON string; valid only for the call's duration
 *
 * Sensitive field values (password, token, text, …) are replaced with
 * `{"redacted":true}` before the message is formatted.
 *
 * Invocations are best-effort. A panicking callback is caught and silently
 * discarded; no command fails because of a trace delivery error. A callback
 * that emits `tracing` events is safe: the recursive `on_event` is dropped
 * by a per-thread guard before it reaches the callback again.
 *
 * # Safety
 *
 * `cb` must be null or a valid function pointer with the declared signature.
 * The pointer is stored atomically; the subscriber may call it from threads
 * other than the registering thread.
 *
 * A callback unregistered via `NULL` may still be invoked from another thread
 * for a brief window after this call returns. The callback (and any data it
 * captures) must remain valid for the process lifetime, or the caller must
 * quiesce all tracing sources before unregistering.
 *
 * If a global tracing subscriber was already installed in the process before
 * the first non-null registration, events may not be delivered.
 */
AdResult ad_set_log_callback(void (*cb)(int32_t level, const char *msg));

/**
 * Triggers the named action on the notification at `index`. Typical
 * action names are those reported in `AdNotificationInfo.actions`
 * (e.g. `"Reply"`, `"Open"`).
 *
 * ## Identity / reorder safety
 *
 * Notification Center reindexes entries on every listing — a new
 * notification arriving (or another one being dismissed) shifts which
 * notification sits at any given `index`. Calling this function with
 * an index obtained from a prior `ad_list_notifications` can therefore
 * press the action button on a different notification than the host
 * intended.
 *
 * `expected_app` and `expected_title` let the host pin the targeted
 * notification to an observed fingerprint. If either pointer is
 * non-null, the row currently at `index` must match that field or the
 * call fails closed with `AD_RESULT_ERR_NOTIFICATION_NOT_FOUND`. Both
 * null preserves the legacy index-only behavior for hosts that do
 * their own reconciliation.
 *
 * # Safety
 * `adapter` must be valid. `action_name` must be a non-null UTF-8
 * C string. `expected_app` and `expected_title` must each be null
 * or a NUL-terminated UTF-8 C string. Invalid UTF-8 in either field
 * is rejected with `AD_RESULT_ERR_INVALID_ARGS` rather than silently
 * treated as "no fingerprint". `out` must be a valid writable
 * `*mut AdActionResult`; on error it is zero-initialized.
 */
AdResult ad_notification_action(const struct AdAdapter *adapter,
                                uint32_t index,
                                const char *expected_app,
                                const char *expected_title,
                                const char *action_name,
                                struct AdActionResult *out);

/**
 * Dismisses the notification at `index`. Indexes are only valid within
 * the response to the most recent `ad_list_notifications` call on this
 * thread — the adapter re-queries internally, so dismissing by a stale
 * index returns `AD_RESULT_ERR_NOTIFICATION_NOT_FOUND`.
 *
 * # Safety
 * `adapter` must be valid. `app_filter` may be null.
 */
AdResult ad_dismiss_notification(const struct AdAdapter *adapter,
                                 uint32_t index,
                                 const char *app_filter);

/**
 * Dismisses every notification matching `app_filter` (null = all apps).
 *
 * Returns two lists: `dismissed_out` carries the notifications that
 * were successfully dismissed; `failed_out` holds error strings for
 * notifications where the platform rejected the dismiss. Partial
 * failures do not set last-error — inspect `failed_out` for details.
 *
 * `failed_out` uses the notification-list handle to stay ABI-consistent
 * with the other list-returning FFI calls; the entries carry the
 * original notification shape with `body` populated by the platform
 * error message.
 *
 * # Safety
 * `adapter` must be valid. `app_filter` may be null. `dismissed_out`
 * and `failed_out` must both be valid writable `*mut *mut AdNotificationList`.
 */
AdResult ad_dismiss_all_notifications(const struct AdAdapter *adapter,
                                      const char *app_filter,
                                      struct AdNotificationList **dismissed_out,
                                      struct AdNotificationList **failed_out);

/**
 * Convenience wrapper: free both lists returned by
 * `ad_dismiss_all_notifications`. Equivalent to calling
 * `ad_notification_list_free` on each; provided for symmetry.
 *
 * # Safety
 * Both arguments must be null or pointers from
 * `ad_dismiss_all_notifications`.
 */
void ad_dismiss_all_notifications_free(struct AdNotificationList *dismissed,
                                       struct AdNotificationList *failed);

/**
 * Lists the notifications currently on-screen.
 *
 * Notification indexes are only stable within a single list response.
 * Pass them straight to `ad_dismiss_notification` /
 * `ad_notification_action` without caching across ticks — the adapter
 * re-queries Notification Center internally on every call.
 *
 * # Safety
 * `adapter` must be valid. `filter` may be null. `out` must be a valid
 * writable `*mut *mut AdNotificationList`. On success `*out` is a
 * non-null handle freed with `ad_notification_list_free`.
 */
AdResult ad_list_notifications(const struct AdAdapter *adapter,
                               const struct AdNotificationFilter *filter,
                               struct AdNotificationList **out);

/**
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_notifications`.
 */
uint32_t ad_notification_list_count(const struct AdNotificationList *list);

/**
 * Borrows a notification entry. Null if `index` is out of range.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_notifications`.
 */
const struct AdNotificationInfo *ad_notification_list_get(const struct AdNotificationList *list,
                                                          uint32_t index);

/**
 * Frees the list and each entry's interior strings.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_notifications`.
 */
void ad_notification_list_free(struct AdNotificationList *list);

/**
 * Finds the first element in `win`'s accessibility tree matching the
 * query and resolves it to an opaque `AdNativeHandle`. The caller owns
 * the handle and must release it with `ad_free_handle(adapter, handle)`
 * once done.
 *
 * Matching is DFS order, first hit wins. All query fields are optional
 * (null = "don't care") and case-insensitive substring matches:
 * - `role` against `AccessibilityNode.role`
 * - `name_substring` against `AccessibilityNode.name`
 * - `value_substring` against `AccessibilityNode.value`
 *
 * The internal tree fetch always sets `include_bounds: true` so
 * `resolve_element_strict` can disambiguate duplicate-label siblings via
 * `bounds_hash`; without bounds on the matched node the resolver falls
 * back to role+name alone and may pick the wrong element.
 *
 * # Safety
 * `adapter`, `win`, and `query` must be valid pointers. `out_handle`
 * must be a valid writable `*mut AdNativeHandle`. On
 * `AD_RESULT_ERR_ELEMENT_NOT_FOUND` the out-handle is zero-initialized.
 */
AdResult ad_find(const struct AdAdapter *adapter,
                 const struct AdWindowInfo *win,
                 const struct AdFindQuery *query,
                 struct AdNativeHandle *out_handle);

/**
 * Reads a single property off a previously-resolved element handle.
 *
 * Supported properties:
 * - `"value"`  — live textual value (text fields, sliders, progress
 *   indicators). Null out-string when the element has no value.
 * - `"bounds"` — JSON-encoded `{"x":..,"y":..,"width":..,"height":..}`.
 *   Null out-string when bounds are unavailable.
 *
 * The returned string must be freed with `ad_free_string`.
 *
 * # Safety
 * `adapter` must be valid. `handle` must be a non-null `AdNativeHandle`
 * produced by the same live adapter and freed before that adapter is destroyed.
 * `property` must be a non-null UTF-8 C string. `out` must be a valid
 * writable `*mut *mut c_char`; it is null-initialized on entry.
 */
AdResult ad_get(const struct AdAdapter *adapter,
                const struct AdNativeHandle *handle,
                const char *property,
                char **out);

/**
 * Checks whether a named boolean state is set on the first element
 * matching `query` inside `win`'s accessibility tree. Intended for
 * the common agent idiom `find → is("focused") → if yes, act`.
 *
 * Supported property names reflect the strings the macOS tree
 * builder actually emits in `AccessibilityNode.states`:
 *
 * - `"focused"` — true when the node carries the `focused` state.
 * - `"disabled"` — true when the adapter surfaced `disabled`.
 * - `"enabled"` — derived: true iff `disabled` is NOT present. There
 *   is no `enabled` string in the adapter output; asking for it
 *   returns the logical negation so agents don't have to invert
 *   themselves.
 *
 * `"selected"`, `"checked"`, and `"expanded"` are not currently
 * emitted by any platform adapter; asking for them returns
 * `AD_RESULT_ERR_INVALID_ARGS` with a diagnostic last-error rather
 * than silently answering `false`. The set will widen as adapters
 * grow support; future additions stay backwards-compatible
 * (unknown → InvalidArgs, known → deterministic answer).
 *
 * On entry `*out` is always cleared to `false` so a caller inspecting
 * the slot after an error sees a predictable sentinel, not whatever
 * was there before. If the query matches nothing, returns
 * `AD_RESULT_ERR_ELEMENT_NOT_FOUND` with `*out` still `false`.
 *
 * # Safety
 * All pointers must be valid. `property` must be a non-null UTF-8
 * C string. `out` must be a valid writable `*mut bool`.
 */
AdResult ad_is(const struct AdAdapter *adapter,
               const struct AdWindowInfo *win,
               const struct AdFindQuery *query,
               const char *property,
               bool *out);

/**
 * Borrowed pointer to the image bytes; valid until the buffer is freed.
 * Returns null if `buf` is null.
 *
 * # Safety
 * `buf` must be null or returned by `ad_screenshot`.
 */
const uint8_t *ad_image_buffer_data(const struct AdImageBuffer *buf);

/**
 * Byte length of the buffer returned by `ad_image_buffer_data`.
 * Always consistent with the actual allocation (no C-mutable mismatch).
 *
 * # Safety
 * `buf` must be null or returned by `ad_screenshot`.
 */
uint64_t ad_image_buffer_size(const struct AdImageBuffer *buf);

/**
 * Pixel width of the image.
 *
 * # Safety
 * `buf` must be null or returned by `ad_screenshot`.
 */
uint32_t ad_image_buffer_width(const struct AdImageBuffer *buf);

/**
 * Pixel height of the image.
 *
 * # Safety
 * `buf` must be null or returned by `ad_screenshot`.
 */
uint32_t ad_image_buffer_height(const struct AdImageBuffer *buf);

/**
 * Encoding format of the image bytes. Defaults to `PNG` on a null
 * handle — callers must still null-check.
 *
 * # Safety
 * `buf` must be null or returned by `ad_screenshot`.
 */
AdImageFormat ad_image_buffer_format(const struct AdImageBuffer *buf);

/**
 * Allocates and returns an opaque `AdImageBuffer`. The handle owns its
 * byte buffer; inspect it through `ad_image_buffer_data` /
 * `ad_image_buffer_size` / `ad_image_buffer_format` / `_width` / `_height`
 * and free it with `ad_image_buffer_free`.
 *
 * # Safety
 * `adapter` and `target` must be valid pointers. `out` must be a valid
 * writable `*mut *mut AdImageBuffer`. On error `*out` is null and
 * last-error is set.
 */
AdResult ad_screenshot(const struct AdAdapter *adapter,
                       const struct AdScreenshotTarget *target,
                       struct AdImageBuffer **out);

/**
 * Frees the image buffer allocated by `ad_screenshot`.
 *
 * # Safety
 * `buf` must be null or a pointer previously returned by `ad_screenshot`.
 * Double-free is undefined behavior.
 */
void ad_image_buffer_free(struct AdImageBuffer *buf);

/**
 * # Safety
 * `adapter` must be valid. `out` must be a valid writable
 * `*mut *mut AdSurfaceList`. Success produces a list handle freed via
 * `ad_surface_list_free`.
 */
AdResult ad_list_surfaces(const struct AdAdapter *adapter, int32_t pid, struct AdSurfaceList **out);

/**
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_surfaces`.
 */
uint32_t ad_surface_list_count(const struct AdSurfaceList *list);

/**
 * Borrow a surface info entry. Null if `index` is out of range.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_surfaces`.
 */
const struct AdSurfaceInfo *ad_surface_list_get(const struct AdSurfaceList *list, uint32_t index);

/**
 * Frees the list and each entry's interior strings.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_surfaces`.
 */
void ad_surface_list_free(struct AdSurfaceList *list);

/**
 * # Safety
 * `tree` must be null or point to a valid `AdNodeTree` previously returned
 * by `flatten_tree` or `ad_get_tree`. After this call the tree is zeroed.
 */
void ad_free_tree(struct AdNodeTree *tree);

/**
 * Snapshots `win`'s accessibility tree into the flat BFS layout
 * described in the types module. The result is written into `*out`
 * and must be freed with `ad_free_tree`. Direct children of any node
 * live contiguously at `nodes[child_start..child_start + child_count]`.
 *
 * `opts.max_depth` caps tree depth. `opts.surface` selects which
 * surface to snapshot (window body, menu, menubar, sheet, popover,
 * alert, or focused subtree); see `AdSnapshotSurface`.
 * `opts.interactive_only` prunes non-interactive nodes; `opts.compact`
 * collapses containers with no semantic payload.
 *
 * # Raw-tree contract
 *
 * This is a **raw adapter tree**, not the snapshot the CLI `snapshot`
 * subcommand returns. Differences the caller must know about:
 *
 * - `ref_id` is always null on every `AdNode`. The FFI surface does
 *   not run `ref_alloc::allocate_refs`; refs are a CLI/JSON pipeline
 *   concern, so agent-facing code that needs them should drive them
 *   externally (resolve via `ad_find` + `ad_free_handle`, or call the
 *   CLI if refs are required).
 * - `include_bounds`, `interactive_only`, and `compact` are honored
 *   after the adapter returns the raw tree, using
 *   `ref_alloc::transform_tree`. Because refs are not allocated here,
 *   the `interactive_only` cut is role-based rather than ref-based;
 *   otherwise the semantics match the CLI snapshot path.
 * - No skeleton/drill-down pipeline is wired through — `skeleton` is
 *   always false on the underlying `TreeOptions`.
 *
 * If parity with the CLI snapshot is important to your consumer,
 * either use `ad_find` + `ad_get` / `ad_is` for point lookups (which
 * bypass tree shape entirely) or invoke the CLI binary for the
 * snapshot call. A future revision may layer a "normalized snapshot"
 * FFI function on top of this raw path.
 *
 * On error `*out` is zeroed so `ad_free_tree` on it is a safe no-op.
 *
 * # Safety
 * All pointers must be non-null. `win.id` and `win.title` must be
 * valid UTF-8 C strings. `out` must be writable.
 */
AdResult ad_get_tree(const struct AdAdapter *adapter,
                     const struct AdWindowInfo *win,
                     const struct AdTreeOptions *opts,
                     struct AdNodeTree *out);

size_t ad_action_size(void);

size_t ad_action_result_size(void);

size_t ad_action_step_size(void);

size_t ad_drag_params_size(void);

size_t ad_element_state_size(void);

size_t ad_ref_entry_size(void);

/**
 * Returns the size of `AdWaitArgs` as compiled. Ctypes and other
 * foreign bindings must call this and compare against their own
 * `sizeof` before passing args to `ad_wait`.
 */
size_t ad_wait_args_size(void);

/**
 * Brings `win` to the foreground on the current space. Returns
 * `AD_RESULT_ERR_WINDOW_NOT_FOUND` when the referenced window no longer
 * exists (the caller should re-list and retry).
 *
 * # Safety
 * `adapter` must be a non-null pointer from `ad_adapter_create`. `win`
 * must be a non-null pointer to an `AdWindowInfo` whose `id` and
 * `title` fields are non-null, valid UTF-8 C strings.
 */
AdResult ad_focus_window(const struct AdAdapter *adapter, const struct AdWindowInfo *win);

/**
 * Releases the heap-allocated string fields (`id`, `title`, `app_name`)
 * inside a single `AdWindowInfo` previously written by `ad_launch_app`
 * or returned through a list accessor. Does not free the `AdWindowInfo`
 * struct itself — that memory is owned by the caller's stack or by the
 * enclosing list.
 *
 * Named `ad_release_window_fields` (not `ad_free_window`) to disambiguate
 * from the now-removed list-free function and make the semantics clear
 * in the header.
 *
 * # Safety
 * `win` must be null or point to a valid `AdWindowInfo` whose string
 * fields were allocated by this crate. Do not call on pointers inside
 * an `AdWindowList` — free the list instead.
 */
void ad_release_window_fields(struct AdWindowInfo *win);

/**
 * # Safety
 * `adapter` must be valid. `out` must be a valid writable
 * `*mut *mut AdWindowList`. `app_filter` may be null or a C string.
 * Success produces a list handle freed via `ad_window_list_free`.
 */
AdResult ad_list_windows(const struct AdAdapter *adapter,
                         const char *app_filter,
                         bool focused_only,
                         struct AdWindowList **out);

/**
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_windows`.
 */
uint32_t ad_window_list_count(const struct AdWindowList *list);

/**
 * Borrow a window info entry. Null if `index` is out of range.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_windows`.
 */
const struct AdWindowInfo *ad_window_list_get(const struct AdWindowList *list, uint32_t index);

/**
 * Frees the list and each entry's interior strings.
 *
 * # Safety
 * `list` must be null or a pointer returned by `ad_list_windows`.
 */
void ad_window_list_free(struct AdWindowList *list);

/**
 * Performs a window-manager operation (`Resize`, `Move`, `Minimize`,
 * `Maximize`, `Restore`) on `win`. Width / height / x / y are consulted
 * only for the variants that use them; other kinds ignore them.
 *
 * An invalid `op.kind` discriminant is rejected with
 * `AD_RESULT_ERR_INVALID_ARGS` before any adapter call.
 *
 * # Safety
 * `adapter` and `win` must be non-null pointers. `win.id` and
 * `win.title` must be non-null valid UTF-8 C strings.
 */
AdResult ad_window_op(const struct AdAdapter *adapter,
                      const struct AdWindowInfo *win,
                      struct AdWindowOp op);

#endif  /* AGENT_DESKTOP_H */

/* C11 ABI layout guards — auto-generated; do not hand-edit.
 * Each sizeof check references the AD_*_SIZE macro defined above so the
 * size literal lives in exactly one place (the Rust source). Alignment
 * and offset values are structurally fixed on all 64-bit targets.        */
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(AdDragParams) == AD_DRAG_PARAMS_SIZE, "AdDragParams ABI size changed");
_Static_assert(_Alignof(AdDragParams) == 8, "AdDragParams ABI alignment changed");
_Static_assert(sizeof(AdAction) == AD_ACTION_SIZE, "AdAction ABI size changed");
_Static_assert(_Alignof(AdAction) == 8, "AdAction ABI alignment changed");
_Static_assert(sizeof(AdElementState) == AD_ELEMENT_STATE_SIZE, "AdElementState ABI size changed");
_Static_assert(_Alignof(AdElementState) == 8, "AdElementState ABI alignment changed");
_Static_assert(sizeof(AdActionStep) == AD_ACTION_STEP_SIZE, "AdActionStep ABI size changed");
_Static_assert(_Alignof(AdActionStep) == 8, "AdActionStep ABI alignment changed");
_Static_assert(offsetof(AdActionStep, label) == 0, "AdActionStep.label offset changed");
_Static_assert(offsetof(AdActionStep, outcome) == 8, "AdActionStep.outcome offset changed");
_Static_assert(sizeof(AdActionResult) == AD_ACTION_RESULT_SIZE, "AdActionResult ABI size changed");
_Static_assert(_Alignof(AdActionResult) == 8, "AdActionResult ABI alignment changed");
_Static_assert(offsetof(AdActionResult, action) == 0, "AdActionResult.action offset changed");
_Static_assert(offsetof(AdActionResult, ref_id) == 8, "AdActionResult.ref_id offset changed");
_Static_assert(offsetof(AdActionResult, post_state) == 16, "AdActionResult.post_state offset changed");
_Static_assert(offsetof(AdActionResult, steps) == 24, "AdActionResult.steps offset changed");
_Static_assert(offsetof(AdActionResult, step_count) == 32, "AdActionResult.step_count offset changed");
_Static_assert(sizeof(AdRefEntry) == AD_REF_ENTRY_SIZE, "AdRefEntry ABI size changed");
_Static_assert(_Alignof(AdRefEntry) == 8, "AdRefEntry ABI alignment changed");
_Static_assert(sizeof(struct AdWaitArgs) == AD_WAIT_ARGS_SIZE, "AdWaitArgs ABI size drift");
_Static_assert(_Alignof(struct AdWaitArgs) == 8, "AdWaitArgs ABI alignment changed");
#endif /* __STDC_VERSION__ >= 201112L */
