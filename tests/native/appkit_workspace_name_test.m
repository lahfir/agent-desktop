#import "../../crates/macos/src/system/appkit_bridge.m"
#import <objc/runtime.h>
#include <assert.h>

@interface InventoryTestApplication : NSRunningApplication
@property(nonatomic, copy) NSString *testName;
@property(nonatomic, copy) NSURL *testExecutable;
@property(nonatomic, copy) NSString *testBundle;
@property(nonatomic) pid_t testPID;
@end

@implementation InventoryTestApplication
- (NSString *)localizedName { return self.testName; }
- (NSURL *)executableURL { return self.testExecutable; }
- (NSString *)bundleIdentifier { return self.testBundle; }
- (pid_t)processIdentifier { return self.testPID; }
- (NSApplicationActivationPolicy)activationPolicy { return NSApplicationActivationPolicyAccessory; }
- (NSDate *)launchDate { return [NSDate dateWithTimeIntervalSince1970:100]; }
@end

static NSArray *testApplications;

@interface NSWorkspace (InventoryNameTest)
- (NSArray *)inventoryTestApplications;
- (NSRunningApplication *)inventoryTestFrontmost;
@end

@implementation NSWorkspace (InventoryNameTest)
- (NSArray *)inventoryTestApplications { return testApplications; }
- (NSRunningApplication *)inventoryTestFrontmost { return nil; }
@end

static InventoryTestApplication *application(NSString *name, NSString *executable, NSString *bundle, pid_t pid) {
    InventoryTestApplication *app = [InventoryTestApplication new];
    app.testName = name;
    app.testExecutable = executable == nil ? nil : [NSURL fileURLWithPath:executable];
    app.testBundle = bundle;
    app.testPID = pid;
    return app;
}

static NSDictionary *snapshot(void) {
    AgentDesktopBytesResult result = agent_desktop_copy_workspace_snapshot_json();
    assert(result.status == 0);
    NSData *data = [NSData dataWithBytes:result.bytes length:result.length];
    agent_desktop_free_bridge_bytes(result.bytes);
    NSDictionary *value = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    assert(value != nil);
    return value;
}

int main(void) {
    @autoreleasepool {
        method_exchangeImplementations(class_getInstanceMethod(NSWorkspace.class, @selector(runningApplications)),
            class_getInstanceMethod(NSWorkspace.class, @selector(inventoryTestApplications)));
        method_exchangeImplementations(class_getInstanceMethod(NSWorkspace.class, @selector(frontmostApplication)),
            class_getInstanceMethod(NSWorkspace.class, @selector(inventoryTestFrontmost)));
        testApplications = @[
            application(@"Approval Box", @"/tmp/ApprovalBox", @"dev.example.approvalbox", 10),
            application(@"", @"/System/FollowUpUI", @"com.apple.FollowUpUI", 11),
            application(nil, @"/tmp/Unnamed", @"dev.example.unnamed", 12),
            application(nil, nil, @"dev.example.bundle", 13)
        ];
        NSArray *records = snapshot()[@"applications"];
        assert(records.count == 4);
        assert([records[0][@"name"] isEqualToString:@"Approval Box"]);
        assert([records[1][@"name"] isEqualToString:@"FollowUpUI"]);
        assert([records[2][@"name"] isEqualToString:@"Unnamed"]);
        assert([records[3][@"name"] isEqualToString:@"dev.example.bundle"]);
        assert([records[1][@"pid"] intValue] == 11);
        assert([records[1][@"bundle_id"] isEqualToString:@"com.apple.FollowUpUI"]);
        testApplications = @[application(nil, nil, nil, 14)];
        AgentDesktopBytesResult missing = agent_desktop_copy_workspace_snapshot_json();
        assert(missing.status == 2);
        assert(missing.bytes == NULL);
        NSString *oversized = [@"x" stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0];
        testApplications = @[application(oversized, @"/tmp/Valid", @"dev.example.valid", 15)];
        AgentDesktopBytesResult invalid = agent_desktop_copy_workspace_snapshot_json();
        assert(invalid.status == 2);
        assert(invalid.bytes == NULL);
        puts("AppKit inventory name regression tests passed");
    }
    return 0;
}
