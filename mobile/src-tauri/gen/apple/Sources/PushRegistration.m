// Push registration for the app target: ask for permission, register with APNs,
// hand the token to the Rust core, and hand it the target of a tapped notification.
//
// The app's delegate is created by tao (the window library under Tauri), not by
// this project, so there is no delegate source to add a method to. Instead this
// adds the one method APNs calls to the delegate's class when the app has finished
// launching, and installs a notification-centre delegate for the foreground and for
// taps. `+load` runs before `main`, so nothing in main.mm needs to know about it.

#import <UIKit/UIKit.h>
#import <UserNotifications/UserNotifications.h>
#import <objc/runtime.h>

// Exported by the Rust core (mobile/src-tauri/src/lib.rs).
extern void smetana_push_token_changed(const char *token);
extern void smetana_push_target(const char *target);

static void smetana_did_register(id self, SEL _cmd, UIApplication *application, NSData *deviceToken) {
    const unsigned char *bytes = (const unsigned char *)deviceToken.bytes;
    NSMutableString *hex = [NSMutableString stringWithCapacity:deviceToken.length * 2];
    for (NSUInteger i = 0; i < deviceToken.length; i++) {
        [hex appendFormat:@"%02x", bytes[i]];
    }
    smetana_push_token_changed(hex.UTF8String);
}

@interface SmetanaPushRegistration : NSObject <UNUserNotificationCenterDelegate>
@end

@implementation SmetanaPushRegistration

+ (void)load {
    [[NSNotificationCenter defaultCenter] addObserverForName:UIApplicationDidFinishLaunchingNotification
                                                      object:nil
                                                       queue:[NSOperationQueue mainQueue]
                                                  usingBlock:^(NSNotification *note) {
        [SmetanaPushRegistration start];
    }];
}

+ (instancetype)shared {
    static SmetanaPushRegistration *instance;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ instance = [SmetanaPushRegistration new]; });
    return instance;
}

+ (void)start {
    Class delegateClass = [UIApplication.sharedApplication.delegate class];
    if (delegateClass != Nil) {
        class_addMethod(delegateClass,
                        @selector(application:didRegisterForRemoteNotificationsWithDeviceToken:),
                        (IMP)smetana_did_register,
                        "v@:@@");
    }
    UNUserNotificationCenter *center = [UNUserNotificationCenter currentNotificationCenter];
    center.delegate = [SmetanaPushRegistration shared];
    [center requestAuthorizationWithOptions:(UNAuthorizationOptionAlert | UNAuthorizationOptionSound | UNAuthorizationOptionBadge)
                          completionHandler:^(BOOL granted, NSError *error) {
        if (granted) {
            dispatch_async(dispatch_get_main_queue(), ^{
                [UIApplication.sharedApplication registerForRemoteNotifications];
            });
        }
    }];
}

- (void)userNotificationCenter:(UNUserNotificationCenter *)center
       willPresentNotification:(UNNotification *)notification
         withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completionHandler {
    completionHandler(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionSound);
}

- (void)userNotificationCenter:(UNUserNotificationCenter *)center
didReceiveNotificationResponse:(UNNotificationResponse *)response
         withCompletionHandler:(void (^)(void))completionHandler {
    id target = response.notification.request.content.userInfo[@"target"];
    if ([target isKindOfClass:[NSString class]]) {
        smetana_push_target([(NSString *)target UTF8String]);
    }
    completionHandler();
}

@end
