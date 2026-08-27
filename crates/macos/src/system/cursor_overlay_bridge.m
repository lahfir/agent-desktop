#import "cursor_overlay_chrome.h"
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
static const CGFloat ADStage = 240.0;
static const CGFloat ADBoxWidth = 32.0;
static const CGFloat ADBoxHeight = 40.0;
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

static CAShapeLayer *ADDartLayer(void) {
    static const CGPoint dart[] = {
        {1.0, 35.0}, {29.6, 17.5}, {12.7, 16.3}, {4.2, 1.6},
    };
    CAShapeLayer *layer = [CAShapeLayer layer];
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, NULL, dart[0].x, dart[0].y);
    for (size_t index = 1; index < sizeof(dart) / sizeof(dart[0]); index += 1) {
        CGPathAddLineToPoint(path, NULL, dart[index].x, dart[index].y);
    }
    CGPathCloseSubpath(path);
    layer.path = path;
    CGPathRelease(path);
    layer.lineWidth = 3.0;
    layer.lineJoin = kCALineJoinRound;
    layer.shadowColor = NSColor.blackColor.CGColor;
    layer.shadowOpacity = 0.32;
    layer.shadowRadius = 5.0;
    layer.shadowOffset = CGSizeMake(2.5, -3.0);
    ADFreezeLayer(layer);
    return layer;
}

static void ADTintPointer(void) {
    const AgentDesktopCursorStyle *style = ADStyle();
    CAShapeLayer *rim = (CAShapeLayer *)ADPointer.sublayers.firstObject;
    CAShapeLayer *dart = (CAShapeLayer *)ADPointer.sublayers.lastObject;
    rim.fillColor = ADColor(style->rim, 1.0);
    rim.strokeColor = ADColor(style->rim, 1.0);
    dart.fillColor = ADColor(style->fill, 1.0);
    dart.strokeColor = ADColor(style->fill, 1.0);
    ADPointer.transform = CATransform3DMakeScale(style->size, style->size, 1.0);
}

static CALayer *ADPointerLayer(void) {
    CALayer *pointer = [CALayer layer];
    pointer.bounds = CGRectMake(0.0, 0.0, ADBoxWidth, ADBoxHeight);
    pointer.anchorPoint = CGPointMake(1.0 / ADBoxWidth, 35.0 / ADBoxHeight);
    pointer.position = CGPointMake(ADTipX, ADTipY);
    ADFreezeLayer(pointer);
    CAShapeLayer *rim = ADDartLayer();
    rim.lineWidth = 6.5;
    rim.shadowOpacity = 0.34;
    [pointer addSublayer:rim];
    [pointer addSublayer:ADDartLayer()];
    return pointer;
}

static void ADMoveCursor(const AgentDesktopCursorFrame *frame, double mainHeight) {
    [ADCursorWindow setFrameOrigin:NSMakePoint(frame->x - ADTipX, mainHeight - frame->y - ADTipY)];
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

void agent_desktop_cursor_overlay_idle(void) {
    @autoreleasepool {
        ADPump(NSApplication.sharedApplication);
    }
}

void agent_desktop_cursor_overlay_stop(void) {
    [ADCursorWindow orderOut:nil];
    [ADBubbleWindow orderOut:nil];
    [ADRipple orderOut:nil];
    ADHighlightStop();
    ADCursorWindow = nil;
    ADPointer = nil;
    ADRipple = nil;
    ADBubbleWindow = nil;
    ADBubbleText = nil;
}

void agent_desktop_cursor_overlay_rest(void) {
    @autoreleasepool {
        NSApplication *app = NSApplication.sharedApplication;
        for (double step = 1.0; step > 0.0; step -= 0.08) {
            ADCursorWindow.alphaValue = step;
            ADBubbleWindow.alphaValue = step;
            ADPump(app);
            [NSThread sleepForTimeInterval:0.012];
        }
        [ADCursorWindow orderOut:nil];
        [ADBubbleWindow orderOut:nil];
        ADCursorWindow.alphaValue = 1.0;
        ADBubbleWindow.alphaValue = 1.0;
    }
}

void agent_desktop_cursor_overlay_hide(void) {
    [ADCursorWindow orderOut:nil];
    [ADBubbleWindow orderOut:nil];
    [ADRipple orderOut:nil];
    ADHighlightStop();
}

void agent_desktop_cursor_overlay_show(void) {
    [ADCursorWindow orderFrontRegardless];
    if (ADBubbleText.stringValue.length > 0) {
        [ADBubbleWindow orderFrontRegardless];
    }
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
            ADTintPointer();
            [ADCursorWindow orderFrontRegardless];

            bool showsBubble = config->label != NULL && config->label[0] != '\0';
            NSString *nextLabel = showsBubble ? @(config->label) : @"";
            bool changedLabel = ![ADBubbleText.stringValue isEqualToString:nextLabel];
            ADBubbleText.stringValue = nextLabel;
            NSRect bubbleFrame = NSMakeRect(config->bubbleX,
                                            mainHeight - config->bubbleY - ADBubbleHeight,
                                            ADBubbleWidth,
                                            ADBubbleHeight);
            bool followsBubble = showsBubble && !changedLabel && ADBubbleWindow.isVisible;
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
