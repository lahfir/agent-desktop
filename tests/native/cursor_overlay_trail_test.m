#import "../../crates/macos/src/system/cursor_overlay_chrome_bridge.m"
#import "../../crates/macos/src/system/cursor_overlay_display_bridge.m"
#import "../../crates/macos/src/system/cursor_overlay_glow_bridge.m"
#import "../../crates/macos/src/system/cursor_overlay_bridge.m"

static void require(bool condition, const char *message) {
    if (!condition) {
        fprintf(stderr, "%s\n", message);
        exit(1);
    }
}

int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSRect desktop = NSMakeRect(0.0, 0.0, 800.0, 600.0);
        NSPoint start = NSMakePoint(NSMidX(desktop) - 20.0, NSMidY(desktop));
        NSPoint end = NSMakePoint(start.x + 40.0, start.y + 20.0);

        ADTrailBeginInFrame(start, desktop);
        ADTrailAppend(end);
        require(ADTrailWindow != nil && ADTrailLayer.path != NULL, "trail must be visible");
        require(ADTrailWindow.frame.size.width < desktop.size.width &&
                    ADTrailWindow.frame.size.height < desktop.size.height,
                "trail window must stay bounded to the path");
        CGRect bounds = CGPathGetBoundingBox(ADTrailLayer.path);
        require(bounds.size.width >= 40.0 && bounds.size.height >= 20.0,
                "trail path must contain the sampled movement");

        ADTrailFinish(0.9);
        CAAnimation *fade = [ADTrailLayer animationForKey:@"agent-drag-trail-fade"];
        require(fade != nil && fabs(fade.duration - 0.9) < 0.001,
                "trail must use the requested fade duration");
        ADTrailStop();
        require(ADTrailWindow == nil && ADTrailLayer == nil && ADTrailPath == NULL,
                "trail stop must release native state");

        agent_desktop_cursor_overlay_drag_begin(start.x, start.y, false);
        require(agent_desktop_cursor_overlay_drag_active(), "drag begin must arm tracking");
        agent_desktop_cursor_overlay_drag_end(end.x, end.y, false);
        require(!agent_desktop_cursor_overlay_drag_active(), "drag cancel must disarm tracking");

        ADCursorWindow = ADWindow(NSMakeRect(0.0, 0.0, ADStage, ADStage));
        ADBubbleWindow = ADWindow(NSMakeRect(0.0, 0.0, ADBubbleWidth, ADBubbleHeight));
        agent_desktop_cursor_overlay_opacity(0.4);
        require(fabs(ADCursorWindow.alphaValue - 0.4) < 0.001 &&
                    fabs(ADBubbleWindow.alphaValue - 0.4) < 0.001,
                "idle fade must dim the pointer and its label together");
        require(!ADCursorWindow.isVisible && !ADBubbleWindow.isVisible,
                "idle fade must never order a hidden cue front");
        agent_desktop_cursor_overlay_rest();
        require(ADCursorWindow.alphaValue == 1.0 && ADBubbleWindow.alphaValue == 1.0,
                "expiry must leave no dimmed opacity for the next cue");
        require(!ADPersistentPose.hasPose, "expiry must clear the retained pose");
        agent_desktop_cursor_overlay_stop();
    }
    return 0;
}
