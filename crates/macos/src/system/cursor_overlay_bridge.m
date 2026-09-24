#import "cursor_overlay_chrome.h"
#import "cursor_overlay_display.h"
#import "cursor_overlay_glow.h"
#import "cursor_overlay_lifecycle.h"
#import <CoreGraphics/CoreGraphics.h>
#import <stdbool.h>
#import <stddef.h>
#import <stdint.h>
typedef struct {
    double x;
    double y;
    double ripple;
} AgentDesktopCursorFrame;
typedef struct {
    double frameSeconds;
    const char *label;
    double bubbleX;
    double bubbleY;
    double target[4];
    double highlightSeconds;
    uint8_t flags;
} AgentDesktopCursorRenderConfig;
static const uint8_t ADReduceMotion = 1 << 2;
static const uint8_t ADHighlightCue = 1 << 3;
static const uint8_t ADLayoutDeferred = 1 << 4;
static const CGFloat ADStage = 240.0;
static const CGFloat ADTipX = 88.0;
static const CGFloat ADTipY = 172.0;
static const CGFloat ADBubbleWidth = 232.0;
static const CGFloat ADBubbleHeight = 38.0;
static const CGFloat ADHighlightPad = 5.0;

static __strong NSWindow *ADCursorWindow = nil;
static __strong CALayer *ADPointer = nil;
static __strong NSWindow *ADRipple = nil;
static __strong NSWindow *ADBubbleWindow = nil;
static __strong NSTextField *ADBubbleText = nil;
static bool ADDragArmed = false;
static bool ADDragStarted = false;
static bool ADDragTrail = false;
static CGPoint ADDragLast = {0.0, 0.0};
static CFTimeInterval ADDragArmDeadline = 0.0;
static CFTimeInterval ADDragDeadline = 0.0;
static ADPersistentCursorPose ADPersistentPose = {0};

static void ADMoveCursor(const AgentDesktopCursorFrame *frame, double mainHeight) {
    [ADCursorWindow setFrameOrigin:NSMakePoint(frame->x - ADTipX, mainHeight - frame->y - ADTipY)];
}

static void ADDragCancel(void) {
    ADDragArmed = false;
    ADDragStarted = false;
    ADTrailStop();
}

static void ADSuppress(void) {
    ADDragCancel();
    [ADCursorWindow orderOut:nil];
    [ADBubbleWindow orderOut:nil];
    [ADRipple orderOut:nil];
    ADGlowHide();
    ADHighlightStop();
}

static bool ADEnsureTargetVisible(void) {
    if (agent_desktop_cursor_overlay_target_visible()) {
        return true;
    }
    ADSuppress();
    return false;
}

static void ADDragPoll(void) {
    if (!ADDragArmed) {
        return;
    }
    CFTimeInterval now = CACurrentMediaTime();
    bool down = CGEventSourceButtonState(kCGEventSourceStateCombinedSessionState,
                                         kCGMouseButtonLeft);
    if (now >= ADDragDeadline || (!ADDragStarted && now >= ADDragArmDeadline)) {
        ADDragCancel();
        return;
    }
    if (!down) {
        if (ADDragStarted) {
            if (ADDragTrail) {
                ADTrailFinish(0.9);
            }
            ADDragArmed = false;
            ADDragStarted = false;
        }
        return;
    }
    CGEventRef event = CGEventCreate(NULL);
    if (event == NULL) {
        return;
    }
    CGPoint point = CGEventGetLocation(event);
    CFRelease(event);
    if (ADDragStarted && CGPointEqualToPoint(point, ADDragLast)) {
        return;
    }
    double mainHeight = CGDisplayBounds(CGMainDisplayID()).size.height;
    if (!ADDragStarted) {
        ADDragStarted = true;
        ADDragLast = point;
        if (ADDragTrail) {
            ADTrailBegin(NSMakePoint(point.x, mainHeight - point.y));
        }
    } else if (ADBubbleWindow.isVisible) {
        NSPoint origin = ADBubbleWindow.frame.origin;
        [ADBubbleWindow setFrameOrigin:NSMakePoint(origin.x + point.x - ADDragLast.x,
                                                   origin.y - point.y + ADDragLast.y)];
    }
    AgentDesktopCursorFrame frame = {.x = point.x, .y = point.y, .ripple = 0.0};
    ADMoveCursor(&frame, mainHeight);
    double target[2];
    if (agent_desktop_cursor_overlay_target_point(target)) {
        ADPersistentCursorPoseMove(&ADPersistentPose,
                                   point,
                                   ADBubbleWindow.frame.origin,
                                   CGPointMake(target[0], target[1]));
    }
    if (ADDragTrail) {
        ADTrailAppend(NSMakePoint(point.x, mainHeight - point.y));
    }
    ADDragLast = point;
}

static NSWindow *ADBubble(void) {
    NSWindow *window = ADWindow(NSMakeRect(0.0, 0.0, ADBubbleWidth, ADBubbleHeight));
    CALayer *surface = window.contentView.layer;
    surface.backgroundColor = NSColor.whiteColor.CGColor;
    surface.cornerRadius = 10.0;
    surface.borderWidth = 1.5;
    surface.borderColor = [NSColor colorWithSRGBRed:0.08 green:0.08 blue:0.09 alpha:1.0].CGColor;
    ADBubbleText = [NSTextField labelWithString:@""];
    ADBubbleText.frame = NSMakeRect(13.0, 7.0, 206.0, 23.0);
    ADBubbleText.textColor = [NSColor colorWithSRGBRed:0.16 green:0.12 blue:0.06 alpha:1.0];
    ADBubbleText.font = [NSFont systemFontOfSize:12.5];
    [window.contentView addSubview:ADBubbleText];
    return window;
}

static void ADRefreshPersistent(void) {
    bool visible = agent_desktop_cursor_overlay_target_visible();
    ADPersistentCursorPresentation presentation =
        ADPersistentCursorPosePresentation(&ADPersistentPose, visible);
    if (presentation == ADPersistentCursorPresentationNone) {
        ADSuppress();
        return;
    }
    double target[2];
    if (!agent_desktop_cursor_overlay_target_point(target)) {
        ADSuppress();
        return;
    }
    double dx = target[0] - ADPersistentPose.target.x;
    double dy = target[1] - ADPersistentPose.target.y;
    double mainHeight = CGDisplayBounds(CGMainDisplayID()).size.height;
    AgentDesktopCursorFrame frame = {
        .x = ADPersistentPose.pointer.x + dx,
        .y = ADPersistentPose.pointer.y + dy,
        .ripple = 0.0,
    };
    ADMoveCursor(&frame, mainHeight);
    double bubble[2];
    if ((presentation & ADPersistentCursorPresentationLabel) != 0
        && !agent_desktop_cursor_overlay_label_position(frame.x, frame.y,
                                                        ADBubbleWidth, ADBubbleHeight, bubble)) {
        ADSuppress();
        return;
    }
    if ((presentation & ADPersistentCursorPresentationLabel) != 0) {
        [ADBubbleWindow setFrameOrigin:NSMakePoint(bubble[0], mainHeight - bubble[1] - ADBubbleHeight)];
    }
    [ADCursorWindow orderFrontRegardless];
    if ((presentation & ADPersistentCursorPresentationLabel) != 0) {
        [ADBubbleWindow orderFrontRegardless];
    }
    ADGlowRefresh();
}

void agent_desktop_cursor_overlay_idle(void) {
    @autoreleasepool {
        static CFTimeInterval checked = 0;
        CFTimeInterval now = CACurrentMediaTime();
        if (ADPersistentCursorPoseShouldRefresh(ADDragArmed)) {
            if (now - checked >= 0.1) {
                ADRefreshPersistent();
                checked = now;
            }
        } else {
            if (now - checked >= 0.1) {
                if (!agent_desktop_cursor_overlay_target_visible()) {
                    ADSuppress();
                } else if (ADPersistentCursorPoseShows(&ADPersistentPose, true)) {
                    ADGlowRefresh();
                }
                checked = now;
            }
            ADDragPoll();
        }
        ADPump(NSApplication.sharedApplication);
    }
}

void agent_desktop_cursor_overlay_stop(void) {
    ADSuppress();
    ADPersistentCursorPoseClear(&ADPersistentPose);
    ADCursorWindow = nil;
    ADPointer = nil;
    ADRipple = nil;
    ADBubbleWindow = nil;
    ADBubbleText = nil;
}

void agent_desktop_cursor_overlay_hide(void) {
    ADPersistentCursorPoseHide(&ADPersistentPose);
    ADSuppress();
}

void agent_desktop_cursor_overlay_opacity(double alpha) {
    ADSetOpacity(ADCursorWindow, ADBubbleWindow, alpha);
}

void agent_desktop_cursor_overlay_rest(void) {
    ADSuppress();
    ADSetOpacity(ADCursorWindow, ADBubbleWindow, 1.0);
    ADPersistentCursorPoseClear(&ADPersistentPose);
}

void agent_desktop_cursor_overlay_drag_begin(double fromX, double fromY, bool trail) {
    @try {
        @autoreleasepool {
            (void)fromX;
            (void)fromY;
            ADDragCancel();
            ADDragArmed = true;
            ADDragTrail = trail;
            CFTimeInterval now = CACurrentMediaTime();
            ADDragArmDeadline = now + 1.0;
            ADDragDeadline = now + 95.0;
        }
    } @catch (NSException *exception) {
        (void)exception;
        ADDragCancel();
    }
}

void agent_desktop_cursor_overlay_drag_end(double toX, double toY, bool completed) {
    @try {
        @autoreleasepool {
            if (!ADDragArmed) {
                return;
            }
            if (!completed) {
                ADDragCancel();
                return;
            }
            double mainHeight = CGDisplayBounds(CGMainDisplayID()).size.height;
            AgentDesktopCursorFrame frame = {.x = toX, .y = toY, .ripple = 0.0};
            ADMoveCursor(&frame, mainHeight);
            if (ADDragTrail) {
                ADTrailAppend(NSMakePoint(toX, mainHeight - toY));
                ADTrailFinish(0.9);
            }
            ADDragArmed = false;
            ADDragStarted = false;
        }
    } @catch (NSException *exception) {
        (void)exception;
        if (ADDragArmed) {
            ADDragCancel();
        }
    }
}

bool agent_desktop_cursor_overlay_drag_active(void) {
    return ADDragArmed;
}

void agent_desktop_cursor_overlay_show(void) {
    ADPersistentCursorPoseShow(&ADPersistentPose);
    ADRefreshPersistent();
}

static void ADHighlightTarget(const AgentDesktopCursorRenderConfig *config, double mainHeight) {
    const double *target = config->target;
    ADHighlightShow(NSMakeRect(target[0] - ADHighlightPad,
                               mainHeight - target[1] - target[3] - ADHighlightPad,
                               target[2] + ADHighlightPad * 2.0,
                               target[3] + ADHighlightPad * 2.0),
                    config->highlightSeconds);
}

bool agent_desktop_cursor_overlay_run(const AgentDesktopCursorFrame *frames,
                                      size_t frameCount,
                                      const AgentDesktopCursorRenderConfig *config) {
    if (frames == NULL || frameCount == 0 || config == NULL || config->frameSeconds <= 0.0) {
        return false;
    }
    @try {
        @autoreleasepool {
            NSApplication *app = NSApplication.sharedApplication;
            [app setActivationPolicy:NSApplicationActivationPolicyAccessory];
            [app finishLaunching];
            double mainHeight = CGDisplayBounds(CGMainDisplayID()).size.height;
            bool reduceMotion = (config->flags & ADReduceMotion) != 0;
            const AgentDesktopCursorFrame *last = &frames[frameCount - 1];

            if (ADCursorWindow == nil) {
                ADCursorWindow = ADWindow(NSMakeRect(0.0, 0.0, ADStage, ADStage));
                ADPointer = ADPointerLayer();
                [ADCursorWindow.contentView.layer addSublayer:ADPointer];
                ADBubbleWindow = ADBubble();
                ADRipple = ADRippleWindow();
            }
            ADTintPointer(ADPointer);
            ADSetOpacity(ADCursorWindow, ADBubbleWindow, 1.0);

            bool showsBubble = config->label != NULL && config->label[0] != '\0';
            NSString *nextLabel = showsBubble ? @(config->label) : @"";
            bool changedLabel = ![ADBubbleText.stringValue isEqualToString:nextLabel];
            ADBubbleText.stringValue = nextLabel;
            NSRect bubbleFrame = NSMakeRect(config->bubbleX,
                                            mainHeight - config->bubbleY - ADBubbleHeight,
                                            ADBubbleWidth,
                                            ADBubbleHeight);
            double targetAnchor[2] = {last->x, last->y};
            agent_desktop_cursor_overlay_target_point(targetAnchor);
            ADPersistentCursorPoseReplace(&ADPersistentPose,
                                          CGPointMake(last->x, last->y),
                                          bubbleFrame.origin,
                                          CGPointMake(targetAnchor[0], targetAnchor[1]),
                                          showsBubble);
            if ((config->flags & ADLayoutDeferred) != 0) {
                ADRefreshPersistent();
                return true;
            }
            if (!ADEnsureTargetVisible()) {
                return true;
            }
            bool followsBubble = showsBubble && !changedLabel && ADBubbleWindow.isVisible;
            [ADCursorWindow orderFrontRegardless];
            ADGlowRefresh();
            [ADRipple setFrameOrigin:NSMakePoint(last->x - ADRippleSize * 0.5,
                                                 mainHeight - last->y - ADRippleSize * 0.5)];
            size_t movementFrameCount = frameCount;
            bool playsRipple = false;
            for (size_t index = 0; index < frameCount; index += 1) {
                if (frames[index].ripple > 0.0) {
                    movementFrameCount = index;
                    playsRipple = true;
                    break;
                }
            }
            bool highlighted = (config->flags & ADHighlightCue) == 0 || reduceMotion;

            for (size_t index = 0; index < movementFrameCount; index += 1) {
                if (index % 6 == 0 && !ADEnsureTargetVisible()) {
                    return true;
                }
                ADMoveCursor(&frames[index], mainHeight);
                if (followsBubble) {
                    [ADBubbleWindow setFrameOrigin:NSMakePoint(
                        bubbleFrame.origin.x + frames[index].x - last->x,
                        bubbleFrame.origin.y - frames[index].y + last->y)];
                }
                ADPump(app);
                if (index + 1 < movementFrameCount) {
                    [NSThread sleepForTimeInterval:config->frameSeconds];
                }
            }
            if (!ADEnsureTargetVisible()) {
                return true;
            }
            if (playsRipple) {
                ADRipplePlay(ADRipple);
                if (!highlighted) {
                    ADHighlightTarget(config, mainHeight);
                    highlighted = true;
                }
            }
            if (!highlighted) {
                ADHighlightTarget(config, mainHeight);
            }

            if (showsBubble) {
                ADShowBubble(ADBubbleText, bubbleFrame, changedLabel && !reduceMotion);
            } else {
                [ADBubbleWindow orderOut:nil];
            }
            return true;
        }
    } @catch (NSException *exception) {
        (void)exception;
        return false;
    }
}
