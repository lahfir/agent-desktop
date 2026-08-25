#import "cursor_overlay_chrome.h"
#import <math.h>

const CGFloat ADRippleSize = 108.0;
static const NSUInteger ADRippleRings = 2;
static const double ADHighlightSeconds = 2.4;

static AgentDesktopCursorStyle ADCurrentStyle = {
    .fill = {1.0, 1.0, 1.0},
    .rim = {0.07, 0.07, 0.09},
    .accent = {0.26, 0.60, 1.0},
    .size = 1.0,
};

static __strong NSWindow *ADHighlightWindow = nil;
static __strong CALayer *ADHighlightBorder = nil;

static double ADEase(double t) {
    return t * t * t * (10.0 + t * (-15.0 + 6.0 * t));
}

static double ADExpoOut(double t, double rate) {
    return 1.0 - pow(2.0, -rate * t);
}

CGColorRef ADColor(const double *rgb, CGFloat alpha) {
    return [NSColor colorWithSRGBRed:rgb[0] green:rgb[1] blue:rgb[2] alpha:alpha].CGColor;
}

const AgentDesktopCursorStyle *ADStyle(void) {
    return &ADCurrentStyle;
}

void agent_desktop_cursor_overlay_style(const AgentDesktopCursorStyle *style) {
    if (style != NULL) {
        ADCurrentStyle = *style;
    }
}

void ADFreezeLayer(CALayer *layer) {
    layer.actions = @{
        @"transform" : NSNull.null,
        @"opacity" : NSNull.null,
        @"position" : NSNull.null,
        @"bounds" : NSNull.null,
        @"hidden" : NSNull.null,
    };
}

NSWindow *ADWindow(NSRect frame) {
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
    ADFreezeLayer(view.layer);
    return window;
}

void ADPump(NSApplication *app) {
    [app updateWindows];
    [[NSRunLoop currentRunLoop]
        runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.0005]];
}

static CAShapeLayer *ADRing(CGFloat radius, BOOL filled) {
    CAShapeLayer *layer = [CAShapeLayer layer];
    layer.bounds = CGRectMake(0.0, 0.0, ADRippleSize, ADRippleSize);
    layer.position = CGPointMake(ADRippleSize * 0.5, ADRippleSize * 0.5);
    CGPathRef path = CGPathCreateWithEllipseInRect(
        CGRectMake(ADRippleSize * 0.5 - radius,
                   ADRippleSize * 0.5 - radius,
                   radius * 2.0,
                   radius * 2.0),
        NULL);
    layer.path = path;
    CGPathRelease(path);
    layer.fillColor = NSColor.clearColor.CGColor;
    layer.strokeColor = NSColor.clearColor.CGColor;
    layer.opacity = 0.0;
    layer.shouldRasterize = NO;
    if (filled) {
        layer.shadowOffset = CGSizeZero;
        layer.shadowRadius = 14.0;
    }
    ADFreezeLayer(layer);
    return layer;
}

NSWindow *ADRippleWindow(void) {
    NSWindow *window = ADWindow(NSMakeRect(0.0, 0.0, ADRippleSize, ADRippleSize));
    [window.contentView.layer addSublayer:ADRing(19.0, YES)];
    for (NSUInteger index = 0; index < ADRippleRings; index += 1) {
        [window.contentView.layer addSublayer:ADRing(ADRippleSize * 0.5 - 4.0, NO)];
    }
    return window;
}

static void ADContactGlow(CAShapeLayer *layer, double progress) {
    double t = MIN(1.0, progress / 0.22);
    double burst = 0.22 + 0.9 * ADExpoOut(t, 7.0);
    layer.fillColor = ADColor(ADCurrentStyle.accent, 1.0);
    layer.shadowColor = ADColor(ADCurrentStyle.accent, 1.0);
    layer.shadowOpacity = (float)(0.55 * pow(1.0 - t, 2.0));
    layer.transform = CATransform3DMakeScale(burst, burst, 1.0);
    layer.opacity = (float)(0.42 * pow(1.0 - t, 2.6));
}

static void ADWaterRing(CAShapeLayer *layer, double progress, NSUInteger index) {
    double u = (progress - (double)index * 0.15) / 0.72;
    if (u <= 0.0 || u >= 1.0) {
        layer.opacity = 0.0;
        return;
    }
    double spread = 0.04 + 0.96 * ADExpoOut(u, 8.5);
    double stretch = 1.0 + 0.09 * pow(1.0 - u, 3.0);
    layer.strokeColor = ADColor(ADCurrentStyle.accent, 1.0);
    layer.lineWidth = 3.4 * pow(1.0 - u, 1.5) + 0.35;
    layer.transform = CATransform3DMakeScale(spread * stretch, spread, 1.0);
    layer.opacity = (float)(pow(1.0 - u, 1.9) * (index == 0 ? 0.95 : 0.55));
}

void ADRippleFrame(NSArray<CALayer *> *layers, double progress) {
    ADContactGlow((CAShapeLayer *)layers.firstObject, progress);
    for (NSUInteger index = 1; index < layers.count; index += 1) {
        ADWaterRing((CAShapeLayer *)layers[index], progress, index - 1);
    }
}

static CAKeyframeAnimation *ADHold(NSString *path, NSArray *values, NSArray *times) {
    CAKeyframeAnimation *animation = [CAKeyframeAnimation animationWithKeyPath:path];
    animation.values = values;
    animation.keyTimes = times;
    animation.duration = ADHighlightSeconds;
    animation.removedOnCompletion = NO;
    animation.fillMode = kCAFillModeForwards;
    animation.timingFunction =
        [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseInEaseOut];
    return animation;
}

void ADHighlightShow(NSRect frame) {
    if (ADHighlightWindow == nil) {
        ADHighlightWindow = ADWindow(frame);
        ADHighlightBorder = [CALayer layer];
        ADHighlightBorder.borderWidth = 2.5;
        ADHighlightBorder.cornerRadius = 8.0;
        ADHighlightBorder.opacity = 0.0;
        ADFreezeLayer(ADHighlightBorder);
        [ADHighlightWindow.contentView.layer addSublayer:ADHighlightBorder];
    }
    ADHighlightBorder.borderColor = ADColor(ADCurrentStyle.accent, 1.0);
    ADHighlightBorder.backgroundColor = ADColor(ADCurrentStyle.accent, 0.10);
    ADHighlightBorder.shadowColor = ADColor(ADCurrentStyle.accent, 1.0);
    ADHighlightBorder.shadowOpacity = 0.5;
    ADHighlightBorder.shadowRadius = 10.0;
    ADHighlightBorder.shadowOffset = CGSizeZero;
    [ADHighlightWindow setFrame:frame display:NO];
    ADHighlightBorder.frame = CGRectMake(0.0, 0.0, frame.size.width, frame.size.height);
    [ADHighlightBorder removeAllAnimations];
    [ADHighlightBorder addAnimation:ADHold(@"opacity",
                                           @[ @0.0, @1.0, @1.0, @0.0 ],
                                           @[ @0.0, @0.05, @0.78, @1.0 ])
                             forKey:@"agent-highlight-fade"];
    [ADHighlightBorder addAnimation:ADHold(@"transform.scale",
                                           @[ @1.08, @0.99, @1.0, @1.0 ],
                                           @[ @0.0, @0.09, @0.16, @1.0 ])
                             forKey:@"agent-highlight-pop"];
    [ADHighlightWindow orderFrontRegardless];
}

void ADHighlightStop(void) {
    [ADHighlightBorder removeAllAnimations];
    [ADHighlightWindow orderOut:nil];
    ADHighlightWindow = nil;
    ADHighlightBorder = nil;
}

void ADRevealBubble(NSApplication *app, NSTextField *text, NSRect final, double frameSeconds) {
    NSWindow *bubble = text.window;
    CALayer *surface = bubble.contentView.layer;
    bubble.alphaValue = 1.0;
    text.alphaValue = 0.0;
    [bubble setFrame:final display:NO];
    [bubble orderFrontRegardless];
    for (double elapsed = 0.0; elapsed < 0.28; elapsed += frameSeconds) {
        double eased = ADEase(MIN(1.0, elapsed / 0.28));
        double scale = 0.94 + 0.06 * eased;
        surface.transform = CATransform3DMakeScale(scale, scale, 1.0);
        text.alphaValue = eased;
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
    }
    surface.transform = CATransform3DIdentity;
    text.alphaValue = 1.0;
}

void ADFadeBubble(NSApplication *app, NSWindow *bubble, double frameSeconds) {
    for (double elapsed = 0.0; elapsed < 0.24; elapsed += frameSeconds) {
        double t = MIN(1.0, elapsed / 0.24);
        bubble.alphaValue = 1.0 - t * t * (3.0 - 2.0 * t);
        ADPump(app);
        [NSThread sleepForTimeInterval:frameSeconds];
    }
    [bubble orderOut:nil];
}
