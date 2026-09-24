#import "../../crates/macos/src/system/cursor_overlay_lifecycle.h"
#import <assert.h>

static ADPersistentCursorPose pose(double x, const char *label) {
    ADPersistentCursorPose value = {0};
    ADPersistentCursorPoseReplace(&value,
                                  CGPointMake(x, 120.0),
                                  CGPointMake(x + 16.0, 140.0),
                                  CGPointMake(x, 120.0),
                                  label[0] != '\0');
    return value;
}

int main(void) {
    ADPersistentCursorPose retained = pose(100.0, "first");
    ADPersistentCursorPresentation presentation =
        ADPersistentCursorPosePresentation(&retained, true);
    assert(presentation == (ADPersistentCursorPresentationPointer
                            | ADPersistentCursorPresentationLabel));
    assert((presentation & (ADPersistentCursorPresentationRipple
                            | ADPersistentCursorPresentationHighlight
                            | ADPersistentCursorPresentationTrail)) == 0);
    assert(!ADPersistentCursorPoseShows(&retained, false));
    assert(ADPersistentCursorPoseShows(&retained, true));

    retained = pose(200.0, "hidden");
    assert(!ADPersistentCursorPoseShows(&retained, false));
    assert(ADPersistentCursorPoseShows(&retained, true));
    assert(retained.pointer.x == 200.0);

    retained = pose(300.0, "latest");
    assert(retained.pointer.x == 300.0);
    assert(retained.showsLabel);

    ADPersistentCursorPoseHide(&retained);
    assert(!ADPersistentCursorPoseShows(&retained, true));
    ADPersistentCursorPoseShow(&retained);
    assert(ADPersistentCursorPoseShows(&retained, true));

    ADPersistentCursorPoseClear(&retained);
    assert(!ADPersistentCursorPoseShows(&retained, true));
    ADPersistentCursorPoseShow(&retained);
    assert(!ADPersistentCursorPoseShows(&retained, true));

    retained = pose(400.0, "idle");
    for (size_t tick = 0; tick < 1000; tick += 1) {
        assert(ADPersistentCursorPoseShows(&retained, true));
    }

    assert(!ADPersistentCursorPoseShouldRefresh(true));
    ADPersistentCursorPoseMove(&retained,
                               CGPointMake(460.0, 180.0),
                               CGPointMake(476.0, 200.0),
                               CGPointMake(460.0, 180.0));
    assert(retained.pointer.x == 460.0);
    assert(retained.requestedVisible);
    return 0;
}
