#import "../../crates/macos/src/system/cursor_overlay_display_bridge.m"
#import <assert.h>

static NSDictionary *window(int pid, int number, CGRect bounds, double alpha) {
    return @{
        (id)kCGWindowOwnerPID: @(pid),
        (id)kCGWindowNumber: @(number),
        (id)kCGWindowBounds: CFBridgingRelease(CGRectCreateDictionaryRepresentation(bounds)),
        (id)kCGWindowAlpha: @(alpha),
    };
}

static void uncertified_transient_and_unrelated_sibling_remain_occluders(void) {
    bool (^isRenderer)(uint32_t) = ^bool(uint32_t pid) { return pid == 30; };
    CGPoint point = CGPointMake(-1200, -400);
    CGRect bounds = CGRectMake(-1440, -900, 1440, 900);
    NSDictionary *owner = window(10, 42, bounds, 1);
    NSMutableDictionary *transient = [window(10, 44, bounds, 1) mutableCopy];
    transient[(id)kCGWindowLayer] = @(kCGPopUpMenuWindowLevel);
    NSDictionary *sibling = window(10, 43, bounds, 1);
    NSMutableDictionary *unrelatedPopup = [sibling mutableCopy];
    unrelatedPopup[(id)kCGWindowLayer] = @(kCGPopUpMenuWindowLevel);
    NSDictionary *foreign = window(20, 99, bounds, 1);

    assert(!ADTargetVisibleInWindows(@[transient, owner], 10, 42, point, isRenderer));
    assert(!ADTargetVisibleInWindows(@[sibling, owner], 10, 42, point, isRenderer));
    assert(!ADTargetVisibleInWindows(@[unrelatedPopup, owner], 10, 42, point, isRenderer));
    assert(!ADTargetVisibleInWindows(@[transient], 10, 42, point, isRenderer));
    assert(!ADTargetVisibleInWindows(@[sibling], 10, 42, point, isRenderer));
    assert(!ADTargetVisibleInWindows(@[foreign, transient, owner], 10, 42, point, isRenderer));
    assert(ADTargetVisibleInWindows(@[owner, transient], 10, 42, point, isRenderer));
}

int main(void) {
    @autoreleasepool {
        uncertified_transient_and_unrelated_sibling_remain_occluders();
        assert(NSEqualRects(ADTopLeftRectAtHeight(NSMakeRect(-1440, 1080, 1440, 900), 1080),
                            NSMakeRect(-1440, -900, 1440, 900)));
        assert(NSEqualRects(ADTopLeftRectAtHeight(NSMakeRect(1920, -900, 1440, 900), 1080),
                            NSMakeRect(1920, 1080, 1440, 900)));
        assert(NSEqualRects(ADTopLeftRectAtHeight(NSMakeRect(0, 0, 1920, 1080), 1080),
                            NSMakeRect(0, 0, 1920, 1080)));
        CGPoint translated = ADTranslatedTargetPoint(CGPointMake(120, 140),
                                                      CGRectMake(100, 100, 400, 300),
                                                      CGRectMake(500, 300, 400, 300));
        assert(CGPointEqualToPoint(translated, CGPointMake(520, 340)));
        CGPoint label = ADLabelPositionInFrame(790, 590, 232, 38,
                                               NSMakeRect(0, 0, 800, 600));
        assert(CGPointEqualToPoint(label, CGPointMake(540, 534)));
        bool (^isRenderer)(uint32_t) = ^bool(uint32_t pid) { return pid == 30 || pid == 31; };
        CGPoint point = CGPointMake(-1200, -400);
        CGRect bounds = CGRectMake(-1440, -900, 1440, 900);
        NSDictionary *target = window(10, 42, bounds, 1);
        NSDictionary *sibling = window(10, 43, bounds, 1);
        NSDictionary *cover = window(20, 99, bounds, 1);
        assert(ADTargetVisibleInWindows(@[target], 10, 42, point, isRenderer));
        assert(!ADTargetVisibleInWindows(@[], 10, 42, point, isRenderer));
        assert(!ADTargetVisibleInWindows(@[sibling, target], 10, 42, point, isRenderer));
        assert(!ADTargetVisibleInWindows(@[cover, target], 10, 42, point, isRenderer));
        assert(!ADTargetVisibleInWindows(@[target], 11, 42, point, isRenderer));
        assert(!ADTargetVisibleInWindows(@[target], 10, 42, CGPointMake(100, 100), isRenderer));
        assert(ADTargetVisibleInWindows(@[window(30, 2, bounds, 1), target], 10, 42, point, isRenderer));
        assert(ADTargetVisibleInWindows(@[window(31, 2, bounds, 1), target], 10, 42, point, isRenderer));
        assert(ADTargetVisibleInWindows(@[window(20, 2, bounds, 0), target], 10, 42, point, isRenderer));
        assert(ADTargetVisibleInWindows(@[window(20, 2, CGRectMake(0, 0, 1920, 1080), 1), target], 10, 42, point, isRenderer));
    }
    return 0;
}
