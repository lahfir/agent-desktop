#import "../../crates/macos/src/system/appkit_bridge.m"
#import <objc/runtime.h>
#include <stdio.h>

@interface InventoryApplication : NSRunningApplication
@property(nonatomic) int32_t fixturePID;
@property(nonatomic, copy) NSString *fixtureName;
@end
@implementation InventoryApplication
- (pid_t)processIdentifier { return self.fixturePID; }
- (NSString *)localizedName { return self.fixtureName; }
- (NSString *)bundleIdentifier { return @"com.example.inventory"; }
- (NSDate *)launchDate { return [NSDate dateWithTimeIntervalSince1970:100]; }
- (NSApplicationActivationPolicy)activationPolicy { return NSApplicationActivationPolicyRegular; }
@end

static NSArray<NSRunningApplication *> *fixtureApplications;
static NSRunningApplication *fixtureFrontmost;
@interface InventoryWorkspace : NSWorkspace
@end
@implementation InventoryWorkspace
- (NSArray<NSRunningApplication *> *)runningApplications { return fixtureApplications; }
- (NSRunningApplication *)frontmostApplication { return fixtureFrontmost; }
@end

static id fixtureWorkspace(id self, SEL command) {
    (void)self; (void)command;
    return [InventoryWorkspace new];
}

static InventoryApplication *application(int32_t pid, NSString *name) {
    InventoryApplication *app = [InventoryApplication new];
    app.fixturePID = pid;
    app.fixtureName = name;
    return app;
}

static void require(bool condition, const char *message) {
    if (!condition) { fprintf(stderr, "%s\n", message); exit(1); }
}

static NSDictionary *snapshot(void) {
    AgentDesktopBytesResult result = agent_desktop_copy_workspace_snapshot_json();
    require(result.status == 0, "documented PID-less record must not poison process inventory");
    NSData *data = [NSData dataWithBytes:result.bytes length:result.length];
    agent_desktop_free_bridge_bytes(result.bytes);
    return [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
}

static void requireRefusal(NSArray<NSRunningApplication *> *apps, NSRunningApplication *frontmost) {
    fixtureApplications = apps;
    fixtureFrontmost = frontmost;
    AgentDesktopBytesResult result = agent_desktop_copy_workspace_snapshot_json();
    require(result.status == 2 && result.length == 0 && result.bytes == NULL,
            "malformed process metadata must still refuse the complete inventory");
}

int main(void) {
    @autoreleasepool {
        Method method = class_getClassMethod([NSWorkspace class], @selector(sharedWorkspace));
        IMP original = method_setImplementation(method, (IMP)fixtureWorkspace);
        InventoryApplication *valid = application(10, @"Synthetic QA");
        fixtureApplications = @[application(-1, nil), valid];
        fixtureFrontmost = valid;
        NSDictionary *result = snapshot();
        NSArray *apps = result[@"applications"];
        require(apps.count == 1 && [apps[0][@"pid"] intValue] == 10,
                "inventory must keep the live process and exclude only PID-less records");
        require([result[@"frontmost_pid"] intValue] == 10, "frontmost generation must stay intact");
        fixtureApplications = @[application(-1, @"No process")];
        fixtureFrontmost = nil;
        require([snapshot()[@"applications"] count] == 0, "PID-less-only inventory must be empty");
        requireRefusal(@[application(0, @"Invalid"), valid], valid);
        requireRefusal(@[application(-2, @"Invalid"), valid], valid);
        requireRefusal(@[application(11, nil), valid], valid);
        requireRefusal(@[application(11, @""), valid], valid);
        NSString *oversized = [@"x" stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0];
        requireRefusal(@[application(11, oversized), valid], valid);
        requireRefusal(@[valid, valid], valid);
        requireRefusal(@[valid], application(-1, @"Invalid frontmost"));
        method_setImplementation(method, original);
        puts("PASS: PID-less inventory and seven malformed metadata refusal controls");
    }
    return 0;
}
