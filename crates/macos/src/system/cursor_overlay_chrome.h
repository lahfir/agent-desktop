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
void ADFreezeLayer(CALayer *layer);
NSWindow *ADRippleWindow(void);
void ADRippleFrame(NSArray<CALayer *> *layers, double progress);
void ADHighlightShow(NSRect frame);
void ADHighlightStop(void);
void ADRevealBubble(NSApplication *app, NSTextField *text, NSRect final, double frameSeconds);
void ADFadeBubble(NSApplication *app, NSWindow *bubble, double frameSeconds);
