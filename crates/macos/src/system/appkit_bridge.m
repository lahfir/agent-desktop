#import <AppKit/AppKit.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <math.h>

typedef struct {
    uint8_t status;
    uint8_t deliveryStarted;
} AgentDesktopTerminateResult;

typedef struct {
    uint8_t status;
    uint8_t *bytes;
    size_t length;
} AgentDesktopBytesResult;

AgentDesktopTerminateResult agent_desktop_terminate_application(
    int32_t pid,
    double expectedLaunchTime,
    uint8_t force
) {
    AgentDesktopTerminateResult result = { .status = 4, .deliveryStarted = 0 };
    @try {
        @autoreleasepool {
            NSRunningApplication *app =
                [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
            if (app == nil) {
                result.status = 1;
                return result;
            }
            NSDate *launchDate = app.launchDate;
            if (launchDate == nil ||
                fabs(launchDate.timeIntervalSince1970 - expectedLaunchTime) > 5.0) {
                result.status = 5;
                return result;
            }
            result.deliveryStarted = 1;
            BOOL accepted = force != 0 ? [app forceTerminate] : [app terminate];
            result.status = accepted ? 0 : 2;
            return result;
        }
    } @catch (NSException *exception) {
        (void)exception;
        result.status = 3;
        return result;
    }
}

uint8_t agent_desktop_ensure_cocoa_multithreaded(void) {
    @try {
        @autoreleasepool {
            if ([NSThread isMultiThreaded]) {
                return 0;
            }
            NSThread *thread = [[NSThread alloc] initWithBlock:^{}];
            if (thread == nil) {
                return 1;
            }
            [thread start];
            uint32_t remaining = 1000;
            while (!thread.isFinished && remaining > 0) {
                usleep(1000);
                remaining -= 1;
            }
            if (!thread.isFinished) {
                return 2;
            }
            return [NSThread isMultiThreaded] ? 0 : 3;
        }
    } @catch (NSException *exception) {
        (void)exception;
        return 4;
    }
}

AgentDesktopBytesResult agent_desktop_copy_workspace_snapshot_json(void) {
    AgentDesktopBytesResult result = { .status = 5, .bytes = NULL, .length = 0 };
    @try {
        @autoreleasepool {
            NSWorkspace *workspace = [NSWorkspace sharedWorkspace];
            NSArray<NSRunningApplication *> *running = workspace.runningApplications;
            if (running == nil || running.count > 8192) {
                result.status = 1;
                return result;
            }
            int32_t frontmostPID = 0;
            id frontmostLaunchTime = [NSNull null];
            NSRunningApplication *frontmost = workspace.frontmostApplication;
            if (frontmost != nil) {
                if (![frontmost isKindOfClass:[NSRunningApplication class]]) {
                    result.status = 2;
                    return result;
                }
                frontmostPID = frontmost.processIdentifier;
                NSDate *frontmostLaunchDate = frontmost.launchDate;
                if (frontmostPID <= 0) {
                    result.status = 2;
                    return result;
                }
                if (frontmostLaunchDate != nil) {
                    double launchTime = frontmostLaunchDate.timeIntervalSince1970;
                    if (!isfinite(launchTime) || launchTime <= 0.0) {
                        result.status = 2;
                        return result;
                    }
                    frontmostLaunchTime = @(launchTime);
                }
            }
            NSMutableArray<NSDictionary *> *records =
                [NSMutableArray arrayWithCapacity:running.count];
            NSMutableSet<NSNumber *> *seen = [NSMutableSet setWithCapacity:running.count];
            for (NSRunningApplication *app in running) {
                if (![app isKindOfClass:[NSRunningApplication class]]) {
                    result.status = 2;
                    return result;
                }
                NSApplicationActivationPolicy policy = app.activationPolicy;
                NSString *policyName = nil;
                switch (policy) {
                    case NSApplicationActivationPolicyRegular:
                        policyName = @"regular";
                        break;
                    case NSApplicationActivationPolicyAccessory:
                        policyName = @"accessory";
                        break;
                    case NSApplicationActivationPolicyProhibited:
                        continue;
                    default:
                        result.status = 2;
                        return result;
                }
                if (policyName == nil) {
                    continue;
                }
                int32_t pid = app.processIdentifier;
                NSString *name = app.localizedName;
                if (pid <= 0 || name == nil || name.length == 0 ||
                    [name lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 16384) {
                    result.status = 2;
                    return result;
                }
                NSNumber *pidNumber = @(pid);
                if ([seen containsObject:pidNumber]) {
                    result.status = 2;
                    return result;
                }
                [seen addObject:pidNumber];
                NSDate *launchDate = app.launchDate;
                id launchTime = [NSNull null];
                if (launchDate != nil) {
                    double seconds = launchDate.timeIntervalSince1970;
                    if (!isfinite(seconds) || seconds <= 0.0) {
                        result.status = 2;
                        return result;
                    }
                    launchTime = @(seconds);
                }
                NSMutableDictionary *record = [@{
                    @"name": name,
                    @"pid": pidNumber,
                    @"launch_time": launchTime,
                    @"activation_policy": policyName,
                } mutableCopy];
                NSString *bundle = app.bundleIdentifier;
                if (bundle != nil) {
                    if ([bundle lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 16384) {
                        result.status = 2;
                        return result;
                    }
                    record[@"bundle_id"] = bundle;
                }
                [records addObject:record];
            }
            NSDictionary *snapshot = @{
                @"applications": records,
                @"frontmost_pid": @(frontmostPID),
                @"frontmost_launch_time": frontmostLaunchTime,
            };
            NSError *error = nil;
            NSData *data = [NSJSONSerialization dataWithJSONObject:snapshot options:0 error:&error];
            if (data == nil || error != nil || data.length > 1048576) {
                result.status = 3;
                return result;
            }
            if (data.length > 0) {
                result.bytes = malloc(data.length);
                if (result.bytes == NULL) {
                    result.status = 4;
                    return result;
                }
                memcpy(result.bytes, data.bytes, data.length);
            }
            result.length = data.length;
            result.status = 0;
            return result;
        }
    } @catch (NSException *exception) {
        (void)exception;
        if (result.bytes != NULL) {
            free(result.bytes);
            result.bytes = NULL;
            result.length = 0;
        }
        result.status = 5;
        return result;
    }
}

void agent_desktop_free_bridge_bytes(uint8_t *bytes) {
    free(bytes);
}
