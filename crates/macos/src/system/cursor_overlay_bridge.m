#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>
#import <stdbool.h>
#import <stddef.h>
#import <stdint.h>
#import <math.h>

typedef struct {
    double frameSeconds;
    const char *label;
    double bubbleX;
    double bubbleY;
    uint8_t flags;
} AgentDesktopCursorRenderConfig;

static const uint8_t ADClickCue = 1 << 1;
static const uint8_t ADReduceMotion = 1 << 2;
static __strong NSWindow *ADCursorWindow = nil;
static __strong CAShapeLayer *ADArrowLayer = nil;
static __strong CAShapeLayer *ADHandPointerLayer = nil;
static __strong NSWindow *ADBubbleWindow = nil;
static __strong NSTextField *ADBubbleText = nil;

static NSWindow *ADWindow(NSRect frame) {
    NSWindow *window = [[NSWindow alloc]
        initWithContentRect:frame
                  styleMask:NSWindowStyleMaskBorderless
                    backing:NSBackingStoreBuffered
                      defer:NO];
    window.opaque = NO;
    window.backgroundColor = NSColor.clearColor;
    window.hasShadow = NO;
    window.ignoresMouseEvents = YES;
    window.releasedWhenClosed = NO;
    window.hidesOnDeactivate = NO;
    window.level = 25;
    window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                NSWindowCollectionBehaviorFullScreenAuxiliary;
    NSView *view = [[NSView alloc]
        initWithFrame:NSMakeRect(0.0, 0.0, frame.size.width, frame.size.height)];
    view.wantsLayer = YES;
    window.contentView = view;
    return window;
}

static CAShapeLayer *ADCursorLayer(void) {
    CAShapeLayer *layer = [CAShapeLayer layer];
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, NULL, 4.0, 35.0);
    CGPathAddLineToPoint(path, NULL, 4.0, 7.0);
    CGPathAddLineToPoint(path, NULL, 11.0, 14.0);
    CGPathAddLineToPoint(path, NULL, 16.0, 3.5);
    CGPathAddLineToPoint(path, NULL, 22.0, 6.5);
    CGPathAddLineToPoint(path, NULL, 17.0, 16.0);
    CGPathAddLineToPoint(path, NULL, 28.0, 16.0);
    CGPathCloseSubpath(path);
    layer.path = path;
    layer.fillColor = [NSColor colorWithSRGBRed:0.98 green:0.99 blue:1.0 alpha:0.98].CGColor;
    layer.strokeColor = [NSColor colorWithSRGBRed:0.04 green:0.05 blue:0.07 alpha:0.94].CGColor;
    layer.lineWidth = 1.8;
    CGPathRelease(path);
    return layer;
}

static CAShapeLayer *ADHandLayer(void) {
    CAShapeLayer *layer = [CAShapeLayer layer];
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, NULL, 8.0, 10.0);
    CGPathAddCurveToPoint(path, NULL, 4.0, 14.0, 3.0, 22.0, 7.0, 24.0);
    CGPathAddCurveToPoint(path, NULL, 9.0, 25.0, 10.0, 24.0, 11.0, 22.0);
    CGPathAddLineToPoint(path, NULL, 11.0, 35.0);
    CGPathAddCurveToPoint(path, NULL, 11.0, 39.0, 17.0, 39.0, 17.0, 35.0);
    CGPathAddLineToPoint(path, NULL, 17.0, 26.0);
    CGPathAddCurveToPoint(path, NULL, 19.0, 30.0, 23.0, 29.0, 23.0, 25.0);
    CGPathAddCurveToPoint(path, NULL, 26.0, 28.0, 30.0, 25.0, 29.0, 21.0);
    CGPathAddLineToPoint(path, NULL, 28.0, 15.0);
    CGPathAddCurveToPoint(path, NULL, 27.0, 10.0, 22.0, 7.0, 16.0, 7.0);
    CGPathCloseSubpath(path);
    layer.path = path;
    layer.fillColor = NSColor.whiteColor.CGColor;
    layer.strokeColor = [NSColor colorWithWhite:0.05 alpha:0.96].CGColor;
    layer.lineWidth = 1.7;
    layer.hidden = YES;
    CGPathRelease(path);
    return layer;
}

static CAShapeLayer *ADRippleLayer(void) {
    CAShapeLayer *layer = [CAShapeLayer layer];
    layer.fillColor = NSColor.clearColor.CGColor;
    layer.strokeColor = [NSColor colorWithSRGBRed:0.78 green:0.81 blue:0.86 alpha:0.88].CGColor;
    layer.lineWidth = 1.4;
    layer.hidden = YES;
    return layer;
}

static void ADUpdateRipple(CAShapeLayer *layer, double progress) {
    if (progress <= 0.0 || progress >= 1.0) {
        layer.hidden = YES;
        return;
    }
    double eased = 1.0 - pow(1.0 - progress, 3.0);
    double radius = 3.0 + 25.0 * eased;
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathAddEllipseInRect(path,
                           NULL,
                           CGRectMake(36.0 - radius,
                                      36.0 - radius,
                                      radius * 2.0,
                                      radius * 2.0));
    layer.path = path;
    layer.opacity = (float)(pow(1.0 - progress, 1.7) * 0.78);
    layer.hidden = NO;
    CGPathRelease(path);
}

static void ADPump(NSApplication *app) {
    [app updateWindows];
    [[NSRunLoop currentRunLoop]
        runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.0005]];
}

void agent_desktop_cursor_overlay_idle(void) {
    @autoreleasepool {
        ADPump(NSApplication.sharedApplication);
    }
}

void agent_desktop_cursor_overlay_stop(void) {
    [ADCursorWindow orderOut:nil];
    [ADBubbleWindow orderOut:nil];
    ADCursorWindow = nil;
    ADArrowLayer = nil;
    ADHandPointerLayer = nil;
    ADBubbleWindow = nil;
    ADBubbleText = nil;
}

void agent_desktop_cursor_overlay_hide(void) {
    [ADCursorWindow orderOut:nil];
    [ADBubbleWindow orderOut:nil];
}

void agent_desktop_cursor_overlay_show(void) {
    [ADCursorWindow orderFrontRegardless];
    if (ADBubbleText.stringValue.length > 0) {
        [ADBubbleWindow orderFrontRegardless];
    }
}

static void ADMoveCursor(NSWindow *window, double x, double y, double mainHeight) {
    [window setFrameOrigin:NSMakePoint(x - 4.0, mainHeight - y - 35.0)];
}

static void ADSetBreathing(CALayer *layer, bool enabled) {
    if (!enabled) {
        [layer removeAnimationForKey:@"agent-breath"];
        layer.transform = CATransform3DIdentity;
        return;
    }
    if ([layer animationForKey:@"agent-breath"] != nil) {
        return;
    }
    CABasicAnimation *breath = [CABasicAnimation animationWithKeyPath:@"transform.scale"];
    breath.fromValue = @0.99;
    breath.toValue = @1.01;
    breath.duration = 1.3;
    breath.autoreverses = YES;
    breath.repeatCount = HUGE_VALF;
    breath.timingFunction =
        [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseInEaseOut];
    [layer addAnimation:breath forKey:@"agent-breath"];
}

static void ADPointerTransition(NSApplication *app,
                                CALayer *arrow,
                                CALayer *hand,
                                bool toHand,
                                double frameSeconds) {
    arrow.hidden = NO;
    hand.hidden = NO;
    double elapsed = 0.0;
    while (elapsed < 0.13) {
        double t = MIN(1.0, elapsed / 0.13);
        double eased = t * t * t * (10.0 + t * (-15.0 + 6.0 * t));
        double handOpacity = toHand ? eased : 1.0 - eased;
        hand.opacity = (float)handOpacity;
        arrow.opacity = (float)(1.0 - handOpacity);
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
        elapsed += frameSeconds;
    }
    hand.hidden = !toHand;
    arrow.hidden = toHand;
    hand.opacity = toHand ? 1.0 : 0.0;
    arrow.opacity = toHand ? 0.0 : 1.0;
}

static void ADRevealBubble(NSApplication *app,
                           NSWindow *bubble,
                           NSTextField *text,
                           NSRect finalFrame,
                           double frameSeconds) {
    CALayer *surface = bubble.contentView.layer;
    bubble.alphaValue = 1.0;
    text.alphaValue = 0.0;
    surface.transform = CATransform3DMakeScale(0.94, 0.94, 1.0);
    [bubble orderFrontRegardless];
    double elapsed = 0.0;
    while (elapsed < 0.28) {
        double t = MIN(1.0, elapsed / 0.28);
        double eased = t * t * t * (10.0 + t * (-15.0 + 6.0 * t));
        NSRect frame = finalFrame;
        frame.origin.y -= 7.0 * (1.0 - eased);
        [bubble setFrame:frame display:YES];
        double scale = 0.94 + 0.06 * eased;
        surface.transform = CATransform3DMakeScale(scale, scale, 1.0);
        text.alphaValue = eased;
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
        elapsed += frameSeconds;
    }
    [bubble setFrame:finalFrame display:YES];
    surface.transform = CATransform3DIdentity;
    text.alphaValue = 1.0;
}

static void ADClick(NSApplication *app,
                    CALayer *cursorLayer,
                    CALayer *handLayer,
                    NSWindow *ripple,
                    CAShapeLayer *first,
                    CAShapeLayer *second,
                    double x,
                    double y,
                    double mainHeight,
                    double frameSeconds) {
    [ripple setFrameOrigin:NSMakePoint(x - 36.0, mainHeight - y - 36.0)];
    [ripple orderFrontRegardless];
    ADPointerTransition(app, cursorLayer, handLayer, true, frameSeconds);
    double elapsed = 0.0;
    while (elapsed < 0.44) {
        double t = elapsed / 0.44;
        ADUpdateRipple(first, t);
        ADUpdateRipple(second, MAX(0.0, (t - 0.2) / 0.8));
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
        elapsed += frameSeconds;
    }
    [ripple orderOut:nil];
    ADPointerTransition(app, cursorLayer, handLayer, false, frameSeconds);
}

static void ADFadeBubble(NSApplication *app,
                         NSWindow *bubble,
                         double frameSeconds) {
    double elapsed = 0.0;
    while (elapsed < 0.24) {
        double t = MIN(1.0, elapsed / 0.24);
        double eased = t * t * (3.0 - 2.0 * t);
        bubble.alphaValue = 1.0 - eased;
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
        elapsed += frameSeconds;
    }
    [bubble orderOut:nil];
}

bool agent_desktop_cursor_overlay_run(const double *points,
                                      size_t pointCount,
                                      const AgentDesktopCursorRenderConfig *config) {
    if (points == NULL || pointCount == 0 || config == NULL || config->frameSeconds <= 0.0) {
        return false;
    }
    @try {
        @autoreleasepool {
            NSApplication *app = NSApplication.sharedApplication;
            [app setActivationPolicy:NSApplicationActivationPolicyAccessory];
            [app finishLaunching];
            CGRect main = CGDisplayBounds(CGMainDisplayID());
            double mainHeight = main.size.height;
            bool click = (config->flags & ADClickCue) != 0;
            bool reduceMotion = (config->flags & ADReduceMotion) != 0;

            if (ADCursorWindow == nil) {
                ADCursorWindow = ADWindow(NSMakeRect(0.0, 0.0, 40.0, 42.0));
                ADArrowLayer = ADCursorLayer();
                [ADCursorWindow.contentView.layer addSublayer:ADArrowLayer];
                ADHandPointerLayer = ADHandLayer();
                [ADCursorWindow.contentView.layer addSublayer:ADHandPointerLayer];
            }
            NSWindow *cursor = ADCursorWindow;
            CAShapeLayer *cursorLayer = ADArrowLayer;
            CAShapeLayer *handLayer = ADHandPointerLayer;
            ADSetBreathing(cursorLayer, !reduceMotion);
            ADSetBreathing(handLayer, !reduceMotion);
            [cursor orderFrontRegardless];

            NSRect bubbleFrame = NSZeroRect;
            bool showsBubble = config->label != NULL && config->label[0] != '\0';
            if (ADBubbleWindow == nil) {
                ADBubbleWindow = ADWindow(NSMakeRect(0.0, 0.0, 232.0, 38.0));
                CALayer *surface = ADBubbleWindow.contentView.layer;
                surface.backgroundColor = NSColor.whiteColor.CGColor;
                surface.cornerRadius = 10.0;
                surface.borderWidth = 1.5;
                surface.borderColor =
                    [NSColor colorWithSRGBRed:0.08 green:0.08 blue:0.09 alpha:1.0].CGColor;
                ADBubbleText = [NSTextField labelWithString:@""];
                ADBubbleText.frame = NSMakeRect(13.0, 7.0, 206.0, 23.0);
                ADBubbleText.textColor =
                    [NSColor colorWithSRGBRed:0.16 green:0.12 blue:0.06 alpha:1.0];
                ADBubbleText.font = [NSFont systemFontOfSize:12.5];
                [ADBubbleWindow.contentView addSubview:ADBubbleText];
            }
            NSString *nextLabel = showsBubble
                ? [NSString stringWithUTF8String:config->label]
                : @"";
            bool changedLabel = ![ADBubbleText.stringValue isEqualToString:nextLabel];
            if (changedLabel && ADBubbleWindow.isVisible) {
                if (reduceMotion) {
                    [ADBubbleWindow orderOut:nil];
                } else {
                    ADFadeBubble(app, ADBubbleWindow, config->frameSeconds);
                }
            }
            ADBubbleText.stringValue = nextLabel;
            if (showsBubble) {
                bubbleFrame = NSMakeRect(config->bubbleX,
                                         mainHeight - config->bubbleY - 38.0,
                                         232.0,
                                         38.0);
            }

            for (size_t index = 0; index < pointCount; index += 1) {
                ADMoveCursor(cursor, points[index * 2], points[index * 2 + 1], mainHeight);
                if (showsBubble && !changedLabel && ADBubbleWindow.isVisible) {
                    NSRect movingFrame = bubbleFrame;
                    movingFrame.origin.x += points[index * 2] - points[(pointCount - 1) * 2];
                    movingFrame.origin.y -= points[index * 2 + 1] - points[(pointCount - 1) * 2 + 1];
                    [ADBubbleWindow setFrame:movingFrame display:YES];
                }
                ADPump(app);
                if (index + 1 < pointCount) {
                    [NSThread sleepForTimeInterval:config->frameSeconds];
                }
            }

            if (showsBubble && changedLabel && !reduceMotion) {
                ADRevealBubble(app,
                               ADBubbleWindow,
                               ADBubbleText,
                               bubbleFrame,
                               config->frameSeconds);
            } else if (showsBubble) {
                [ADBubbleWindow setFrame:bubbleFrame display:YES];
                ADBubbleWindow.alphaValue = 1.0;
                ADBubbleText.alphaValue = 1.0;
                [ADBubbleWindow orderFrontRegardless];
            }
            if (click && !reduceMotion) {
                NSWindow *ripple = ADWindow(NSMakeRect(0.0, 0.0, 72.0, 72.0));
                CAShapeLayer *first = ADRippleLayer();
                CAShapeLayer *second = ADRippleLayer();
                [ripple.contentView.layer addSublayer:first];
                [ripple.contentView.layer addSublayer:second];
                ADClick(app,
                        cursorLayer,
                        handLayer,
                        ripple,
                        first,
                        second,
                        points[(pointCount - 1) * 2],
                        points[(pointCount - 1) * 2 + 1],
                        mainHeight,
                        config->frameSeconds);
            } else if (click) {
                cursorLayer.hidden = YES;
                handLayer.hidden = NO;
                ADPump(app);
                [NSThread sleepForTimeInterval:0.12];
                handLayer.hidden = YES;
                cursorLayer.hidden = NO;
            }
            return true;
        }
    } @catch (NSException *exception) {
        (void)exception;
        return false;
    }
}
