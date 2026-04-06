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

enum AdWindowOpKind {
  AD_WINDOW_OP_KIND_RESIZE = 0,
  AD_WINDOW_OP_KIND_MOVE = 1,
  AD_WINDOW_OP_KIND_MINIMIZE = 2,
  AD_WINDOW_OP_KIND_MAXIMIZE = 3,
  AD_WINDOW_OP_KIND_RESTORE = 4,
};
typedef int32_t AdWindowOpKind;

typedef struct AdAdapter AdAdapter;

typedef struct AdRefEntry {
  int32_t pid;
  const char *role;
  const char *name;
  uint64_t bounds_hash;
  bool has_bounds_hash;
} AdRefEntry;

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

typedef struct AdAppInfo {
  const char *name;
  int32_t pid;
  const char *bundle_id;
} AdAppInfo;

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

typedef struct AdMouseEvent {
  AdMouseEventKind kind;
  struct AdPoint point;
  AdMouseButton button;
  uint32_t click_count;
} AdMouseEvent;

typedef struct AdScreenshotTarget {
  AdScreenshotKind kind;
  uint64_t screen_index;
  int32_t pid;
} AdScreenshotTarget;

typedef struct AdImageBuffer {
  const uint8_t *data;
  uint64_t data_len;
  AdImageFormat format;
  uint32_t width;
  uint32_t height;
} AdImageBuffer;

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
} AdTreeOptions;

typedef struct AdWindowOp {
  AdWindowOpKind kind;
  double width;
  double height;
  double x;
  double y;
} AdWindowOp;

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
 * # Safety
 *
 * `result` must be a pointer to an `AdActionResult` previously written by `ad_execute_action`,
 * or null. After this call all pointers inside the struct are invalid.
 */
void ad_free_action_result(struct AdActionResult *result);

/**
 * # Safety
 * `adapter` must be a valid pointer from `ad_adapter_create`.
 * `out` and `out_count` must be valid writable pointers.
 */
AdResult ad_list_apps(const struct AdAdapter *adapter, struct AdAppInfo **out, uint32_t *out_count);

/**
 * # Safety
 * `apps` must be null or a pointer previously returned by `ad_list_apps`.
 */
void ad_free_apps(struct AdAppInfo *apps, uint32_t count);

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
 * `adapter` must be valid. `id` must be a valid C string.
 */
AdResult ad_close_app(const struct AdAdapter *adapter, const char *id, bool force);

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
 * `event` must be a non-null pointer to a valid `AdMouseEvent`.
 */
AdResult ad_mouse_event(const struct AdAdapter *adapter, const struct AdMouseEvent *event);

/**
 * # Safety
 *
 * `adapter` must be a non-null pointer returned by `ad_adapter_create`.
 * `params` must be a non-null pointer to a valid `AdDragParams`.
 */
AdResult ad_drag(const struct AdAdapter *adapter, const struct AdDragParams *params);

/**
 * # Safety
 * `adapter` and `target` must be valid. `out` must be writable.
 */
AdResult ad_screenshot(const struct AdAdapter *adapter,
                       const struct AdScreenshotTarget *target,
                       struct AdImageBuffer *out);

/**
 * # Safety
 * `img` must be null or point to an `AdImageBuffer` from `ad_screenshot`.
 */
void ad_free_image(struct AdImageBuffer *img);

/**
 * # Safety
 * `adapter` must be valid. `out` and `out_count` must be writable.
 */
AdResult ad_list_surfaces(const struct AdAdapter *adapter,
                          int32_t pid,
                          struct AdSurfaceInfo **out,
                          uint32_t *out_count);

/**
 * # Safety
 * `surfaces` must be null or from `ad_list_surfaces`.
 */
void ad_free_surfaces(struct AdSurfaceInfo *surfaces, uint32_t count);

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
 * `adapter` must be valid. `out` and `out_count` must be writable.
 */
AdResult ad_list_windows(const struct AdAdapter *adapter,
                         const char *app_filter,
                         struct AdWindowInfo **out,
                         uint32_t *out_count);

/**
 * # Safety
 * `windows` must be null or from `ad_list_windows`.
 */
void ad_free_windows(struct AdWindowInfo *windows, uint32_t count);

/**
 * # Safety
 * `win` must be null or point to a valid `AdWindowInfo`.
 */
void ad_free_window(struct AdWindowInfo *win);

/**
 * # Safety
 * `adapter` and `win` must be valid pointers.
 */
AdResult ad_focus_window(const struct AdAdapter *adapter, const struct AdWindowInfo *win);

/**
 * # Safety
 * `adapter` and `win` must be valid pointers.
 */
AdResult ad_window_op(const struct AdAdapter *adapter,
                      const struct AdWindowInfo *win,
                      struct AdWindowOp op);

#endif  /* AGENT_DESKTOP_H */
