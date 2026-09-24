#ifndef AGENT_DESKTOP_CURSOR_OVERLAY_LIFECYCLE_H
#define AGENT_DESKTOP_CURSOR_OVERLAY_LIFECYCLE_H

#import <AppKit/AppKit.h>
#import <stdbool.h>

typedef NS_OPTIONS(uint8_t, ADPersistentCursorPresentation) {
    ADPersistentCursorPresentationNone = 0,
    ADPersistentCursorPresentationPointer = 1 << 0,
    ADPersistentCursorPresentationLabel = 1 << 1,
    ADPersistentCursorPresentationRipple = 1 << 2,
    ADPersistentCursorPresentationHighlight = 1 << 3,
    ADPersistentCursorPresentationTrail = 1 << 4,
};

typedef struct {
    CGPoint pointer;
    CGPoint bubble;
    CGPoint target;
    bool hasPose;
    bool requestedVisible;
    bool showsLabel;
} ADPersistentCursorPose;

static inline void ADPersistentCursorPoseReplace(ADPersistentCursorPose *pose,
                                                  CGPoint pointer,
                                                  CGPoint bubble,
                                                  CGPoint target,
                                                  bool showsLabel) {
    pose->pointer = pointer;
    pose->bubble = bubble;
    pose->target = target;
    pose->hasPose = true;
    pose->requestedVisible = true;
    pose->showsLabel = showsLabel;
}

static inline void ADPersistentCursorPoseMove(ADPersistentCursorPose *pose,
                                               CGPoint pointer,
                                               CGPoint bubble,
                                               CGPoint target) {
    pose->pointer = pointer;
    pose->bubble = bubble;
    pose->target = target;
}

static inline bool ADPersistentCursorPoseShouldRefresh(bool dragActive) {
    return !dragActive;
}

static inline void ADPersistentCursorPoseHide(ADPersistentCursorPose *pose) {
    pose->requestedVisible = false;
}

static inline void ADPersistentCursorPoseShow(ADPersistentCursorPose *pose) {
    pose->requestedVisible = true;
}

static inline void ADPersistentCursorPoseClear(ADPersistentCursorPose *pose) {
    *pose = (ADPersistentCursorPose){0};
}

static inline ADPersistentCursorPresentation ADPersistentCursorPosePresentation(
    const ADPersistentCursorPose *pose, bool targetVisible) {
    if (!pose->hasPose || !pose->requestedVisible || !targetVisible) {
        return ADPersistentCursorPresentationNone;
    }
    return ADPersistentCursorPresentationPointer
        | (pose->showsLabel ? ADPersistentCursorPresentationLabel : 0);
}

static inline bool ADPersistentCursorPoseShows(const ADPersistentCursorPose *pose,
                                                bool targetVisible) {
    return ADPersistentCursorPosePresentation(pose, targetVisible)
        != ADPersistentCursorPresentationNone;
}

#endif
