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

static NSRect ADTopLeftRect(NSRect frame) {
    CGRect main = CGDisplayBounds(CGMainDisplayID());
    return NSMakeRect(frame.origin.x,
                      main.size.height - NSMaxY(frame),
                      frame.size.width,
                      frame.size.height);
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

static void ADMoveCursor(NSWindow *window, double x, double y, double mainHeight) {
    [window setFrameOrigin:NSMakePoint(x - 4.0, mainHeight - y - 35.0)];
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

static void ADSettle(NSApplication *app,
                     CALayer *cursorLayer,
                     double frameSeconds) {
    double elapsed = 0.0;
    while (elapsed < 0.48) {
        double breath = 0.5 + 0.5 * sin(elapsed * M_PI * 2.0 / 0.72);
        double scale = 0.99 + 0.01 * breath;
        cursorLayer.transform = CATransform3DMakeScale(scale, scale, 1.0);
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
        elapsed += frameSeconds;
    }
    cursorLayer.transform = CATransform3DIdentity;
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

            NSWindow *cursor = ADWindow(NSMakeRect(0.0, 0.0, 40.0, 42.0));
            CAShapeLayer *cursorLayer = ADCursorLayer();
            [cursor.contentView.layer addSublayer:cursorLayer];
            CAShapeLayer *handLayer = ADHandLayer();
            [cursor.contentView.layer addSublayer:handLayer];
            [cursor orderFrontRegardless];

            NSWindow *bubble = nil;
            NSTextField *text = nil;
            NSRect bubbleFrame = NSZeroRect;
            bool showsBubble = config->label != NULL && config->label[0] != '\0';
            if (showsBubble) {
                bubbleFrame = NSMakeRect(config->bubbleX,
                                         mainHeight - config->bubbleY - 38.0,
                                         232.0,
                                         38.0);
                bubble = ADWindow(bubbleFrame);
                CALayer *surface = bubble.contentView.layer;
                surface.backgroundColor = NSColor.whiteColor.CGColor;
                surface.cornerRadius = 10.0;
                surface.borderWidth = 1.5;
                surface.borderColor =
                    [NSColor colorWithSRGBRed:0.08 green:0.08 blue:0.09 alpha:1.0].CGColor;
                text = [NSTextField labelWithString:[NSString stringWithUTF8String:config->label]];
                text.frame = NSMakeRect(13.0, 7.0, 206.0, 23.0);
                text.textColor = [NSColor colorWithSRGBRed:0.16 green:0.12 blue:0.06 alpha:1.0];
                text.font = [NSFont systemFontOfSize:12.5];
                [bubble.contentView addSubview:text];
            }

            for (size_t index = 0; index < pointCount; index += 1) {
                ADMoveCursor(cursor, points[index * 2], points[index * 2 + 1], mainHeight);
                ADPump(app);
                if (index + 1 < pointCount) {
                    [NSThread sleepForTimeInterval:config->frameSeconds];
                }
            }

            if (showsBubble && !reduceMotion) {
                ADRevealBubble(app, bubble, text, bubbleFrame, config->frameSeconds);
            } else if (showsBubble) {
                [bubble orderFrontRegardless];
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
            if (!reduceMotion) {
                ADSettle(app, cursorLayer, config->frameSeconds);
            } else {
                ADPump(app);
                [NSThread sleepForTimeInterval:0.12];
            }
            [cursor orderOut:nil];
            [bubble orderOut:nil];
            return true;
        }
    } @catch (NSException *exception) {
        (void)exception;
        return false;
    }
}
