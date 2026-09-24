#import "cursor_overlay_chrome.h"
#import "cursor_overlay_display.h"
#import "cursor_overlay_glow.h"

static const CGFloat ADGlowLine = 2.0;
static const CGFloat ADGlowInset = 2.0;

static __strong NSWindow *ADGlowWindow = nil;
static __strong CALayer *ADGlowBorder = nil;
static double ADGlowAlpha = 1.0;
static bool ADGlowShown = false;

/// Upper bound on the target window's corner radius. The outline is clipped to
/// a rounded rect of this radius at the window edge, so it never paints into
/// the corner gaps outside the target's own rounded shape. macOS 26 windows are
/// rounder (up to about 26 pt with a toolbar); binaries built against an older
/// SDK see macOS 26 reported as 16, so either version selects the larger bound.
static CGFloat ADGlowCornerRadius(void) {
    NSOperatingSystemVersion rounder = {16, 0, 0};
    return [NSProcessInfo.processInfo isOperatingSystemAtLeastVersion:rounder] ? 26.0 : 10.0;
}

static NSWindow *ADGlowMakeWindow(void) {
    NSWindow *window = ADWindow(NSMakeRect(0.0, 0.0, 1.0, 1.0));
    window.alphaValue = 0.0;
    window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorTransient
        | NSWindowCollectionBehaviorIgnoresCycle
        | NSWindowCollectionBehaviorFullScreenAuxiliary;
    window.contentView.layer.masksToBounds = YES;
    ADGlowBorder = [CALayer layer];
    ADGlowBorder.borderWidth = ADGlowLine;
    ADGlowBorder.shadowOpacity = 0.5;
    ADGlowBorder.shadowRadius = 5.0;
    ADGlowBorder.shadowOffset = CGSizeZero;
    [window.contentView.layer addSublayer:ADGlowBorder];
    return window;
}

/// Fits the outline to the target's AppKit frame at the target's window level.
/// The line is inset and concentric with the clip, so it stays inside the
/// target however the radius bound compares with the real corner.
static void ADGlowLayout(NSRect frame, NSInteger level) {
    const AgentDesktopCursorStyle *style = ADStyle();
    CGFloat radius = ADGlowCornerRadius();
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    ADGlowWindow.level = level;
    if (!NSEqualRects(ADGlowWindow.frame, frame)) {
        [ADGlowWindow setFrame:frame display:NO];
    }
    ADGlowWindow.contentView.layer.cornerRadius = radius;
    ADGlowBorder.frame = NSInsetRect(ADGlowWindow.contentView.bounds, ADGlowInset, ADGlowInset);
    ADGlowBorder.cornerRadius = radius - ADGlowInset;
    ADGlowBorder.borderColor = ADColor(style->accent, 0.6);
    ADGlowBorder.shadowColor = ADColor(style->accent, 1.0);
    [CATransaction commit];
}

/// AppKit only documents relative ordering against window numbers it lists,
/// so a target outside that list is never used as an ordering anchor.
static bool ADGlowCanOrderAbove(uint32_t window) {
    NSArray<NSNumber *> *numbers =
        [NSWindow windowNumbersWithOptions:NSWindowNumberListAllApplications];
    return [numbers containsObject:@(window)];
}

/// The outline becomes visible only after a window list confirms its
/// placement; until then it is ordered at zero opacity, so a relative ordering
/// the window server ignored or applied elsewhere is never seen. The window
/// server applies an ordering asynchronously, so a list read straight after
/// ordering never contains it: the confirmation happens on the next refresh.
void ADGlowRefresh(void) {
    if (ADGlowWindow == nil) {
        ADGlowWindow = ADGlowMakeWindow();
    }
    uint32_t glow = (uint32_t)MAX(ADGlowWindow.windowNumber, 0);
    ADGlowTarget target = {0};
    NSRect frame = NSZeroRect;
    ADGlowPlacement placement = ADTargetGlowPlacement(glow, &target, &frame);
    if (placement == ADGlowPlacementUnordered && ADGlowCanOrderAbove(target.window)) {
        ADGlowShown = false;
        ADGlowWindow.alphaValue = 0.0;
        ADGlowLayout(frame, target.level);
        [ADGlowWindow orderWindow:NSWindowAbove relativeTo:(NSInteger)target.window];
        return;
    }
    if (placement != ADGlowPlacementPlaced) {
        ADGlowHide();
        return;
    }
    ADGlowLayout(frame, target.level);
    ADGlowShown = true;
    ADGlowWindow.alphaValue = ADGlowAlpha;
}

void ADGlowHide(void) {
    ADGlowShown = false;
    ADGlowWindow.alphaValue = 0.0;
    [ADGlowWindow orderOut:nil];
}

void ADGlowSetOpacity(double alpha) {
    ADGlowAlpha = alpha;
    if (ADGlowShown) {
        ADGlowWindow.alphaValue = alpha;
    }
}
