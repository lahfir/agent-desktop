#pragma once

#import <AppKit/AppKit.h>
#import <stdbool.h>
#import <stdint.h>

/// Whether the target window outline may be drawn this tick.
///
/// `Hidden`: the exact target window is not on screen, or a visible foreign
/// window above it overlaps its bounds, so the outline would paint over that
/// window. `Unordered`: the outline window is not immediately above the target
/// in window-server order and must be reordered and re-checked before it may
/// become visible. `Placed`: the outline sits immediately above the target
/// (renderer-owned or fully transparent windows aside) and nothing foreign
/// overlaps the target from above.
typedef NS_ENUM(uint8_t, ADGlowPlacement) {
    ADGlowPlacementHidden,
    ADGlowPlacementUnordered,
    ADGlowPlacementPlaced,
};

/// The exact target window the outline is bound to. `pid` and `window` are
/// inputs; `level` and `bounds` (CG global, top-left origin) are filled from
/// the window-server record of that window.
typedef struct {
    uint32_t pid;
    uint32_t window;
    NSInteger level;
    CGRect bounds;
} ADGlowTarget;

bool agent_desktop_cursor_overlay_target_visible(void);
bool agent_desktop_cursor_overlay_target_point(double *output);
bool agent_desktop_cursor_overlay_label_position(double x, double y, double width,
                                                 double height, double *output);
ADGlowPlacement ADTargetGlowPlacement(uint32_t glow, ADGlowTarget *target, NSRect *frame);
