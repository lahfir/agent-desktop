#import "cursor_overlay_display.h"
#import <CoreGraphics/CoreGraphics.h>

static uint32_t ADTargetPid = 0;
static uint32_t ADTargetWindow = 0;
static CGPoint ADTargetPoint;
static CGRect ADTargetBounds;
static bool ADTargetBoundsKnown = false;
static CGPoint ADVisibleTargetPoint;

static bool ADWindowBoundsInWindows(NSArray *windows, uint32_t pid, uint32_t number,
                                    CGRect *output) {
    for (NSDictionary *window in windows) {
        if ([window[(id)kCGWindowOwnerPID] unsignedIntValue] != pid
            || [window[(id)kCGWindowNumber] unsignedIntValue] != number) {
            continue;
        }
        return CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)window[(id)kCGWindowBounds], output);
    }
    return false;
}

static CGPoint ADTranslatedTargetPoint(CGPoint point, CGRect from, CGRect to) {
    return CGPointMake(point.x + to.origin.x - from.origin.x,
                       point.y + to.origin.y - from.origin.y);
}

static bool ADTargetVisibleInWindows(NSArray *windows, uint32_t pid, uint32_t number,
                                     CGPoint point, bool (^isRenderer)(uint32_t)) {
    for (NSDictionary *window in windows) {
        uint32_t owner = [window[(id)kCGWindowOwnerPID] unsignedIntValue];
        if ([window[(id)kCGWindowAlpha] doubleValue] <= 0) {
            continue;
        }
        CGRect bounds;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)window[(id)kCGWindowBounds], &bounds)
            || !CGRectContainsPoint(bounds, point)) {
            continue;
        }
        if (isRenderer(owner)) {
            continue;
        }
        return owner == pid && [window[(id)kCGWindowNumber] unsignedIntValue] == number;
    }
    return false;
}

/// Treats this renderer and peer renderers of the same executable (other
/// agents' cursors) as transparent to target visibility and outline overlap.
static bool ADRendererOwner(uint32_t owner) {
    NSRunningApplication *renderer = NSRunningApplication.currentApplication;
    if (owner == (uint32_t)renderer.processIdentifier) {
        return true;
    }
    NSRunningApplication *peer = [NSRunningApplication runningApplicationWithProcessIdentifier:(pid_t)owner];
    return renderer.executableURL != nil && [renderer.executableURL isEqual:peer.executableURL];
}

/// Classifies the outline window `glow` against the front-to-back on-screen
/// window list. Only windows in front of the target matter: the outline is
/// clipped to the target's own bounds, so anything behind the target is
/// already covered by it. Any visible foreign window in front of the target
/// that overlaps it hides the outline outright, even when ordering would keep
/// the outline beneath that window, so a stale ordering can never let it paint
/// over another app. Records without readable bounds are treated as overlaps.
static ADGlowPlacement ADGlowPlacementInWindows(NSArray *windows, uint32_t glow,
                                                bool (^isRenderer)(uint32_t),
                                                ADGlowTarget *target) {
    NSUInteger targetIndex = NSNotFound;
    for (NSUInteger index = 0; index < windows.count; index += 1) {
        if ([windows[index][(id)kCGWindowOwnerPID] unsignedIntValue] == target->pid
            && [windows[index][(id)kCGWindowNumber] unsignedIntValue] == target->window) {
            targetIndex = index;
            break;
        }
    }
    if (targetIndex == NSNotFound
        || !CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)windows[targetIndex][(id)kCGWindowBounds], &target->bounds)) {
        return ADGlowPlacementHidden;
    }
    target->level = [windows[targetIndex][(id)kCGWindowLayer] integerValue];
    bool glowAbove = false;
    for (NSUInteger index = 0; index < targetIndex; index += 1) {
        NSDictionary *window = windows[index];
        if ([window[(id)kCGWindowNumber] unsignedIntValue] == glow) {
            glowAbove = true;
            continue;
        }
        if ([window[(id)kCGWindowAlpha] doubleValue] <= 0) {
            continue;
        }
        CGRect bounds;
        bool readable = CGRectMakeWithDictionaryRepresentation(
            (__bridge CFDictionaryRef)window[(id)kCGWindowBounds], &bounds);
        bool overlaps = !readable || !CGRectIsEmpty(CGRectIntersection(bounds, target->bounds));
        if ((!overlaps && !glowAbove) || isRenderer([window[(id)kCGWindowOwnerPID] unsignedIntValue])) {
            continue;
        }
        if (overlaps) {
            return ADGlowPlacementHidden;
        }
        glowAbove = false;
    }
    return glowAbove ? ADGlowPlacementPlaced : ADGlowPlacementUnordered;
}

void agent_desktop_cursor_overlay_target(uint32_t pid, uint32_t window, double x, double y) {
    ADTargetPid = pid;
    ADTargetWindow = window;
    ADTargetPoint = CGPointMake(x, y);
    ADVisibleTargetPoint = ADTargetPoint;
    ADTargetBoundsKnown = false;
    if (pid == 0 || window == 0) {
        return;
    }
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
        kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
    ADTargetBoundsKnown = ADWindowBoundsInWindows(windows, pid, window, &ADTargetBounds);
}

bool agent_desktop_cursor_overlay_target_visible(void) {
    if (ADTargetPid == 0) {
        return true;
    }
    @autoreleasepool {
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:(pid_t)ADTargetPid];
        if (app == nil || app.hidden || app.terminated || ADTargetWindow == 0) {
            return false;
        }
        NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
        CGRect currentBounds;
        if (!ADWindowBoundsInWindows(windows, ADTargetPid, ADTargetWindow, &currentBounds)) {
            return false;
        }
        ADVisibleTargetPoint = ADTargetBoundsKnown
            ? ADTranslatedTargetPoint(ADTargetPoint, ADTargetBounds, currentBounds)
            : ADTargetPoint;
        return ADTargetVisibleInWindows(windows, ADTargetPid, ADTargetWindow, ADVisibleTargetPoint,
                                        ^bool(uint32_t owner) { return ADRendererOwner(owner); });
    }
}

bool agent_desktop_cursor_overlay_target_point(double *output) {
    if (output == NULL) {
        return false;
    }
    output[0] = ADVisibleTargetPoint.x;
    output[1] = ADVisibleTargetPoint.y;
    return true;
}

static NSRect ADTopLeftRectAtHeight(NSRect frame, double mainHeight) {
    return NSMakeRect(frame.origin.x,
                      mainHeight - NSMaxY(frame),
                      frame.size.width,
                      frame.size.height);
}

static NSRect ADTopLeftRect(NSRect frame) {
    return ADTopLeftRectAtHeight(frame, CGDisplayBounds(CGMainDisplayID()).size.height);
}

/// CG global rects (top-left origin) and AppKit global rects (bottom-left
/// origin) convert with the same flip about the main display height, so this
/// is the inverse of `ADTopLeftRect` across every display.
static NSRect ADAppKitRect(CGRect bounds) {
    return ADTopLeftRect(NSRectFromCGRect(bounds));
}

/// Classifies the outline window `glow` against the live on-screen window
/// list for the current exact target and writes the target's AppKit frame.
ADGlowPlacement ADTargetGlowPlacement(uint32_t glow, ADGlowTarget *target, NSRect *frame) {
    target->pid = ADTargetPid;
    target->window = ADTargetWindow;
    if (ADTargetPid == 0 || ADTargetWindow == 0) {
        return ADGlowPlacementHidden;
    }
    @autoreleasepool {
        NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
        ADGlowPlacement placement = ADGlowPlacementInWindows(
            windows, glow, ^bool(uint32_t owner) { return ADRendererOwner(owner); }, target);
        *frame = ADAppKitRect(target->bounds);
        return placement;
    }
}

static NSScreen *ADScreenAt(double x, double y) {
    NSPoint point = NSMakePoint(x, y);
    for (NSScreen *screen in NSScreen.screens) {
        if (NSPointInRect(point, ADTopLeftRect(screen.frame))) {
            return screen;
        }
    }
    return nil;
}

static CGPoint ADLabelPositionInFrame(double x, double y, double width, double height,
                                          NSRect frame) {
    double right = NSMaxX(frame);
    double bottom = NSMaxY(frame);
    double placedX = x + 18.0 + width <= right ? x + 18.0 : x - width - 18.0;
    double placedY = y + 18.0 + height <= bottom ? y + 18.0 : y - height - 18.0;
    return CGPointMake(MAX(frame.origin.x, MIN(placedX, MAX(right - width, frame.origin.x))),
                       MAX(frame.origin.y, MIN(placedY, MAX(bottom - height, frame.origin.y))));
}

bool agent_desktop_cursor_overlay_label_position(double x, double y, double width,
                                                 double height, double *output) {
    if (output == NULL) {
        return false;
    }
    NSScreen *screen = ADScreenAt(x, y);
    if (screen == nil) {
        return false;
    }
    CGPoint position = ADLabelPositionInFrame(x, y, width, height,
                                              ADTopLeftRect(screen.visibleFrame));
    output[0] = position.x;
    output[1] = position.y;
    return true;
}

bool agent_desktop_cursor_overlay_screen(double x,
                                         double y,
                                         double *output) {
    if (output == NULL) {
        return false;
    }
    @try {
        @autoreleasepool {
            NSScreen *screen = ADScreenAt(x, y);
            if (screen == nil) {
                return false;
            }
            NSRect frame = ADTopLeftRect(screen.visibleFrame);
            output[0] = frame.origin.x;
            output[1] = frame.origin.y;
            output[2] = frame.size.width;
            output[3] = frame.size.height;
            double refreshRate = 60.0;
            NSNumber *screenNumber = screen.deviceDescription[@"NSScreenNumber"];
            if (screenNumber != nil) {
                CGDisplayModeRef mode = CGDisplayCopyDisplayMode(screenNumber.unsignedIntValue);
                if (mode != NULL) {
                    double reportedRate = CGDisplayModeGetRefreshRate(mode);
                    if (reportedRate > 0.0) {
                        refreshRate = reportedRate;
                    }
                    CGDisplayModeRelease(mode);
                }
            }
            output[4] = MAX(60.0, MIN(120.0, refreshRate));
            output[5] = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion ? 1.0 : 0.0;
            return true;
        }
    } @catch (NSException *exception) {
        (void)exception;
        return false;
    }
}
