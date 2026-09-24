#import "../../crates/macos/src/system/cursor_overlay_chrome_bridge.m"
#import "../../crates/macos/src/system/cursor_overlay_display_bridge.m"
#import "../../crates/macos/src/system/cursor_overlay_glow_bridge.m"
#import "../../crates/macos/src/system/cursor_overlay_bridge.m"

static const uint32_t Glow = 7;

static void require(bool condition, const char *message) {
    if (!condition) {
        fprintf(stderr, "%s\n", message);
        exit(1);
    }
}

static NSDictionary *record(int pid, int number, CGRect bounds, double alpha) {
    return @{
        (id)kCGWindowOwnerPID: @(pid),
        (id)kCGWindowNumber: @(number),
        (id)kCGWindowBounds: CFBridgingRelease(CGRectCreateDictionaryRepresentation(bounds)),
        (id)kCGWindowAlpha: @(alpha),
    };
}

static ADGlowPlacement place(NSArray *windows, ADGlowTarget *target) {
    bool (^isRenderer)(uint32_t) = ^bool(uint32_t pid) { return pid == 30 || pid == 31; };
    *target = (ADGlowTarget){.pid = 10, .window = 42};
    return ADGlowPlacementInWindows(windows, Glow, isRenderer, target);
}

static void placement_requires_the_outline_directly_above_the_exact_target(void) {
    CGRect bounds = CGRectMake(-1440, -900, 1440, 900);
    NSMutableDictionary *owner = [record(10, 42, bounds, 1) mutableCopy];
    owner[(id)kCGWindowLayer] = @3;
    NSDictionary *target = owner;
    NSDictionary *glow = record(30, Glow, bounds, 0);
    NSDictionary *away = record(20, 99, CGRectMake(0, 0, 400, 300), 1);
    ADGlowTarget found;

    require(place(@[glow, target], &found) == ADGlowPlacementPlaced,
            "an outline directly above the target may show");
    require(CGRectEqualToRect(found.bounds, bounds) && found.level == 3,
            "the outline takes the target's bounds and window level");
    require(place(@[target], &found) == ADGlowPlacementUnordered,
            "an outline that is not ordered in must be ordered first");
    require(place(@[target, glow], &found) == ADGlowPlacementUnordered,
            "an outline behind the target must be reordered");
    require(place(@[glow, away, target], &found) == ADGlowPlacementUnordered,
            "a foreign window between the outline and the target breaks the ordering");
    require(place(@[away, glow, target], &found) == ADGlowPlacementPlaced,
            "a non-overlapping window above the outline is irrelevant");
    require(place(@[glow, record(31, 5, bounds, 1), target], &found) == ADGlowPlacementPlaced,
            "another agent's outline between them is allowed");
    require(place(@[glow, record(20, 99, bounds, 0), target], &found) == ADGlowPlacementPlaced,
            "a fully transparent window between them is allowed");
    require(place(@[glow, record(10, 43, bounds, 1)], &found) == ADGlowPlacementHidden,
            "a sibling window never stands in for the exact target");
    require(place(@[glow], &found) == ADGlowPlacementHidden,
            "an off-screen target hides the outline");
}

static void overlapping_foreign_windows_suppress_the_outline(void) {
    CGRect bounds = CGRectMake(100, 25, 800, 600);
    NSDictionary *target = record(10, 42, bounds, 1);
    NSDictionary *glow = record(30, Glow, bounds, 1);
    NSDictionary *cover = record(20, 99, CGRectMake(700, 400, 400, 400), 1);
    NSDictionary *popup = record(10, 44, CGRectMake(200, 100, 200, 300), 1);
    NSDictionary *menuBar = record(20, 1, CGRectMake(0, 0, 1440, 25), 1);
    NSMutableDictionary *unreadable = [record(20, 98, bounds, 1) mutableCopy];
    [unreadable removeObjectForKey:(id)kCGWindowBounds];
    ADGlowTarget found;

    require(place(@[cover, glow, target], &found) == ADGlowPlacementHidden,
            "a foreign window covering part of the target hides the outline");
    require(place(@[glow, cover, target], &found) == ADGlowPlacementHidden,
            "an overlapping window between them hides the outline");
    require(place(@[popup, glow, target], &found) == ADGlowPlacementHidden,
            "the target app's own popup over the target hides the outline");
    require(place(@[unreadable, glow, target], &found) == ADGlowPlacementHidden,
            "a window with unknown bounds counts as an overlap");
    require(place(@[record(30, 3, bounds, 1), glow, target], &found) == ADGlowPlacementPlaced,
            "the renderer's own cursor windows are not overlaps");
    require(place(@[menuBar, glow, target], &found) == ADGlowPlacementPlaced,
            "a window that only touches the target edge is not an overlap");
    require(place(@[glow, target, cover], &found) == ADGlowPlacementPlaced,
            "windows behind the target are covered by it");
}

static void outline_frame_follows_the_target_through_the_display_transform(void) {
    CGRect secondary = CGRectMake(-1440, -900, 1440, 900);
    require(NSEqualRects(ADTopLeftRectAtHeight(NSRectFromCGRect(secondary), 1080),
                         NSMakeRect(-1440, 1080, 1440, 900)),
            "CG bounds on a display above the main one map to AppKit coordinates");
    CGRect main = CGRectMake(100, 25, 800, 600);
    NSRect frame = ADAppKitRect(main);
    double mainHeight = CGDisplayBounds(CGMainDisplayID()).size.height;
    require(NSEqualRects(frame, NSMakeRect(100, mainHeight - 625, 800, 600)),
            "the outline frame is the target's CG bounds flipped about the main display");

    ADGlowWindow = ADGlowMakeWindow();
    ADGlowLayout(NSMakeRect(100, 200, 800, 600), 3);
    require(NSEqualRects(ADGlowWindow.frame, NSMakeRect(100, 200, 800, 600)),
            "the outline window covers exactly the target frame");
    require(ADGlowWindow.level == 3, "the outline shares the target's window level");
    require(CGRectEqualToRect(ADGlowBorder.frame, CGRectMake(2, 2, 796, 596)),
            "the outline line is inset inside the target");
    require(ADGlowWindow.contentView.layer.masksToBounds
                && ADGlowBorder.cornerRadius + ADGlowInset
                       == ADGlowWindow.contentView.layer.cornerRadius,
            "the outline is clipped to a rounded rect concentric with the line");
    ADGlowLayout(NSMakeRect(300, 260, 640, 480), 0);
    require(NSEqualRects(ADGlowWindow.frame, NSMakeRect(300, 260, 640, 480))
                && CGRectEqualToRect(ADGlowBorder.frame, CGRectMake(2, 2, 636, 476)),
            "the outline follows a moved and resized target");
    require(!ADGlowWindow.isVisible, "layout never orders the outline in");
}

static void outline_window_is_click_through_and_never_key(void) {
    require(ADGlowWindow.ignoresMouseEvents, "the outline must be click-through");
    require(!ADGlowWindow.canBecomeKeyWindow && !ADGlowWindow.canBecomeMainWindow,
            "the outline must never become key or main");
    require((ADGlowWindow.collectionBehavior & NSWindowCollectionBehaviorTransient) != 0
                && (ADGlowWindow.collectionBehavior & NSWindowCollectionBehaviorIgnoresCycle) != 0,
            "the outline stays out of Mission Control and window cycling");
}

static void outline_opacity_follows_the_shared_fade_and_clears(void) {
    agent_desktop_cursor_overlay_opacity(0.4);
    require(ADGlowWindow.alphaValue == 0.0 && !ADGlowWindow.isVisible,
            "the fade never reveals an outline that is not placed");
    ADGlowShown = true;
    agent_desktop_cursor_overlay_opacity(0.4);
    require(fabs(ADGlowWindow.alphaValue - 0.4) < 0.001,
            "a placed outline dims with the pointer and label");
    require(!ADGlowWindow.isVisible, "the fade never orders the outline in");
    agent_desktop_cursor_overlay_rest();
    require(!ADGlowShown && ADGlowWindow.alphaValue == 0.0 && !ADGlowWindow.isVisible,
            "expiry clears the outline");
    require(ADGlowAlpha == 1.0, "expiry leaves no dimmed opacity for the next cue");
    ADGlowShown = true;
    agent_desktop_cursor_overlay_stop();
    require(!ADGlowShown && !ADGlowWindow.isVisible, "stop clears the outline");
    ADGlowShown = true;
    agent_desktop_cursor_overlay_hide();
    require(!ADGlowShown && !ADGlowWindow.isVisible, "hide clears the outline");
}

static void outline_stays_hidden_without_a_visible_exact_target(void) {
    agent_desktop_cursor_overlay_target(0, 0, 10, 10);
    ADGlowRefresh();
    require(!ADGlowShown && !ADGlowWindow.isVisible, "an untargeted cue never outlines a window");
    agent_desktop_cursor_overlay_target((uint32_t)getpid(), 0x7ffffff0, 10, 10);
    ADGlowRefresh();
    require(!ADGlowShown && !ADGlowWindow.isVisible, "a target window that is not on screen hides the outline");
    ADPersistentCursorPoseReplace(&ADPersistentPose, CGPointMake(10, 10), CGPointMake(20, 20),
                                  CGPointMake(10, 10), false);
    ADGlowShown = true;
    agent_desktop_cursor_overlay_show();
    require(!ADGlowShown && !ADGlowWindow.isVisible, "show for a hidden target keeps the outline hidden");
    agent_desktop_cursor_overlay_rest();
}

int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        placement_requires_the_outline_directly_above_the_exact_target();
        overlapping_foreign_windows_suppress_the_outline();
        outline_frame_follows_the_target_through_the_display_transform();
        outline_window_is_click_through_and_never_key();
        outline_opacity_follows_the_shared_fade_and_clears();
        outline_stays_hidden_without_a_visible_exact_target();
    }
    return 0;
}
