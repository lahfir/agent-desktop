#pragma once

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

extern const CGFloat ADRippleSize;

typedef struct {
    double fill[3];
    double rim[3];
    double accent[3];
    double size;
} AgentDesktopCursorStyle;

CGColorRef ADColor(const double *rgb, CGFloat alpha);
const AgentDesktopCursorStyle *ADStyle(void);

NSWindow *ADWindow(NSRect frame);
void ADPump(NSApplication *app);
void ADSetOpacity(NSWindow *pointer, NSWindow *bubble, double alpha);
void ADFreezeLayer(CALayer *layer);
CALayer *ADPointerLayer(void);
void ADTintPointer(CALayer *pointer);
NSWindow *ADRippleWindow(void);
void ADRipplePlay(NSWindow *window);
void ADHighlightShow(NSRect frame, double seconds);
void ADHighlightStop(void);
void ADTrailBegin(NSPoint point);
void ADTrailAppend(NSPoint point);
void ADTrailFinish(double seconds);
void ADTrailStop(void);
void ADShowBubble(NSTextField *text, NSRect frame, bool changed);
