#ifndef AGENT_DESKTOP_H
#define AGENT_DESKTOP_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

enum AdActionKind {
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
typedef int32_t AdActionKind;

enum AdDirection {
  AD_DIRECTION_UP = 0,
  AD_DIRECTION_DOWN = 1,
  AD_DIRECTION_LEFT = 2,
  AD_DIRECTION_RIGHT = 3,
};
typedef int32_t AdDirection;

enum AdImageFormat {
  AD_IMAGE_FORMAT_PNG = 0,
  AD_IMAGE_FORMAT_JPG = 1,
};
typedef int32_t AdImageFormat;

enum AdModifier {
  AD_MODIFIER_CMD = 0,
  AD_MODIFIER_CTRL = 1,
  AD_MODIFIER_ALT = 2,
  AD_MODIFIER_SHIFT = 3,
};
typedef int32_t AdModifier;

enum AdMouseButton {
  AD_MOUSE_BUTTON_LEFT = 0,
  AD_MOUSE_BUTTON_RIGHT = 1,
  AD_MOUSE_BUTTON_MIDDLE = 2,
};
typedef int32_t AdMouseButton;

enum AdMouseEventKind {
  AD_MOUSE_EVENT_KIND_MOVE = 0,
  AD_MOUSE_EVENT_KIND_DOWN = 1,
  AD_MOUSE_EVENT_KIND_UP = 2,
  AD_MOUSE_EVENT_KIND_CLICK = 3,
};
typedef int32_t AdMouseEventKind;

enum AdResult {
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
};
typedef int32_t AdResult;

enum AdScreenshotKind {
  AD_SCREENSHOT_KIND_SCREEN = 0,
  AD_SCREENSHOT_KIND_WINDOW = 1,
  AD_SCREENSHOT_KIND_FULL_SCREEN = 2,
};
typedef int32_t AdScreenshotKind;

enum AdSnapshotSurface {
  AD_SNAPSHOT_SURFACE_WINDOW = 0,
  AD_SNAPSHOT_SURFACE_FOCUSED = 1,
  AD_SNAPSHOT_SURFACE_MENU = 2,
  AD_SNAPSHOT_SURFACE_MENUBAR = 3,
  AD_SNAPSHOT_SURFACE_SHEET = 4,
  AD_SNAPSHOT_SURFACE_POPOVER = 5,
  AD_SNAPSHOT_SURFACE_ALERT = 6,
};
typedef int32_t AdSnapshotSurface;

enum AdWindowOpKind {
  AD_WINDOW_OP_KIND_RESIZE = 0,
  AD_WINDOW_OP_KIND_MOVE = 1,
  AD_WINDOW_OP_KIND_MINIMIZE = 2,
  AD_WINDOW_OP_KIND_MAXIMIZE = 3,
  AD_WINDOW_OP_KIND_RESTORE = 4,
};
typedef int32_t AdWindowOpKind;

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

typedef struct AdScrollParams {
  AdDirection direction;
  uint32_t amount;
} AdScrollParams;

typedef struct AdKeyCombo {
  const char *key;
  const AdModifier *modifiers;
  uint32_t modifier_count;
} AdKeyCombo;

typedef struct AdPoint {
  double x;
  double y;
} AdPoint;

typedef struct AdDragParams {
  struct AdPoint from;
  struct AdPoint to;
  uint64_t duration_ms;
} AdDragParams;

typedef struct AdAction {
  AdActionKind kind;
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

typedef struct AdActionResult {
  const char *action;
  const char *ref_id;
  struct AdElementState *post_state;
} AdActionResult;

typedef struct AdRefEntry {
  int32_t pid;
  const char *role;
  const char *name;
  uint64_t bounds_hash;
  bool has_bounds_hash;
} AdRefEntry;

typedef struct AdRect {
  double x;
  double y;
  double width;
  double height;
} AdRect;

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

typedef struct AdMouseEvent {
  AdMouseEventKind kind;
  struct AdPoint point;
  AdMouseButton button;
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

typedef struct AdScreenshotTarget {
  AdScreenshotKind kind;
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

typedef struct AdTreeOptions {
  uint8_t max_depth;
  bool include_bounds;
  bool interactive_only;
  bool compact;
  AdSnapshotSurface surface;
} AdTreeOptions;

typedef struct AdWindowOp {
  AdWindowOpKind kind;
  double width;
  double height;
  double x;
  double y;
} AdWindowOp;

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `handle` must be a non-null pointer to a valid `AdNativeHandle`.
 * `action` must be a non-null pointer to a valid `AdAction`.
 * `out` must be a non-null pointer to an `AdActionResult` to write the result into.
 */
AdResult ad_execute_action(const struct AdAdapter *adapter,
                           const struct AdNativeHandle *handle,
                           const struct AdAction *action,
                           struct AdActionResult *out);

/**
 * Releases a handle previously returned by `ad_resolve_element`.
 *
 * On macOS this calls `CFRelease` on the underlying `AXUIElementRef`,
 * balancing the `CFRetain` that happened during `ad_resolve_element`.
 * On Windows/Linux the call is a no-op that returns `AD_RESULT_OK`
 * (platform adapters inherit the default `not_supported` impl, which
 * the FFI surface rewrites to `Ok` here so callers can apply the same
 * release pattern everywhere).
 *
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `handle` must be null or a pointer previously populated by
 * `ad_resolve_element`. Double-free is undefined behavior.
 */
AdResult ad_free_handle(const struct AdAdapter *adapter, const struct AdNativeHandle *handle);

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
 * `result` must be a pointer to an `AdActionResult` previously written by `ad_execute_action`,
 * or null. After this call all pointers inside the struct are invalid.
 */
void ad_free_action_result(struct AdActionResult *result);

struct AdAdapter *ad_adapter_create(void);

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
 * # Safety
 * `adapter` must be valid. `id` must be a valid C string.
 */
AdResult ad_close_app(const struct AdAdapter *adapter, const char *id, bool force);

/**
 * # Safety
 * `adapter` must be valid. `id` must be a valid C string. `out` must be writable.
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
 */
AdResult ad_last_error_code(void);

const char *ad_last_error_message(void);

const char *ad_last_error_suggestion(void);

const char *ad_last_error_platform_detail(void);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `out` must be a non-null pointer to a `*mut c_char` to receive the allocated string.
 * Free the result with `ad_free_string`.
 */
AdResult ad_get_clipboard(const struct AdAdapter *adapter, char **out);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `text` must be a non-null, valid UTF-8 C string.
 */
AdResult ad_set_clipboard(const struct AdAdapter *adapter, const char *text);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 */
AdResult ad_clear_clipboard(const struct AdAdapter *adapter);

/**
 * # Safety
 *
 * `s` must be a pointer previously returned by `ad_get_clipboard`, or null.
 * After this call the pointer is invalid and must not be used.
 */
void ad_free_string(char *s);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `params` must be a non-null pointer to a valid `AdDragParams`.
 */
AdResult ad_drag(const struct AdAdapter *adapter, const struct AdDragParams *params);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `event` must be a non-null pointer to a valid `AdMouseEvent`.
 */
AdResult ad_mouse_event(const struct AdAdapter *adapter, const struct AdMouseEvent *event);

/**
 * Triggers the named action on the notification at `index`. Typical
 * action names are those reported in `AdNotificationInfo.actions`
 * (e.g. `"Reply"`, `"Open"`).
 *
 * # Safety
 * `adapter` must be valid. `action_name` must be a non-null UTF-8
 * C string. `out` must be a valid writable `*mut AdActionResult`;
 * on error it is zero-initialized.
 */
AdResult ad_notification_action(const struct AdAdapter *adapter,
                                uint32_t index,
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
 * # Safety
 * All pointers must be valid. `out` must be writable.
 */
AdResult ad_get_tree(const struct AdAdapter *adapter,
                     const struct AdWindowInfo *win,
                     const struct AdTreeOptions *opts,
                     struct AdNodeTree *out);

/**
 * # Safety
 * `adapter` and `win` must be valid pointers.
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
 * # Safety
 * `adapter` and `win` must be valid pointers.
 */
AdResult ad_window_op(const struct AdAdapter *adapter,
                      const struct AdWindowInfo *win,
                      struct AdWindowOp op);

#endif  /* AGENT_DESKTOP_H */
