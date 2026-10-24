#import <Foundation/Foundation.h>
#import <UserNotifications/UserNotifications.h>
#import <AppKit/AppKit.h>

static void (*dglClicked)(const char *);
@interface DglNotificationDelegate : NSObject <UNUserNotificationCenterDelegate>
@end
@implementation DglNotificationDelegate
- (void)userNotificationCenter:(UNUserNotificationCenter *)center
      didReceiveNotificationResponse:(UNNotificationResponse *)response
      withCompletionHandler:(void (^)(void))completion {
    if ([response.actionIdentifier isEqualToString:UNNotificationDefaultActionIdentifier]) {
        if (dglClicked) dglClicked(response.notification.request.identifier.UTF8String);
        dispatch_async(dispatch_get_main_queue(), ^{ [NSApp activateIgnoringOtherApps:YES]; });
    }
    completion();
}
- (void)userNotificationCenter:(UNUserNotificationCenter *)center
      willPresentNotification:(UNNotification *)notification
      withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completion {
    // Rust already suppresses notifications for panels actually visible in the foreground.
    completion(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionSound);
}
@end
static DglNotificationDelegate *delegate;
bool dgl_notifications_init(void (*callback)(const char *)) {
    if (!NSBundle.mainBundle.bundleIdentifier) return false;
    dglClicked = callback;
    if (!delegate) {
        delegate = [DglNotificationDelegate new];
        UNUserNotificationCenter.currentNotificationCenter.delegate = delegate;
    }
    return true;
}
void dgl_notifications_authorize(void) {
    [UNUserNotificationCenter.currentNotificationCenter requestAuthorizationWithOptions:
        UNAuthorizationOptionAlert | UNAuthorizationOptionSound
        completionHandler:^(BOOL granted, NSError *error) { (void)granted; (void)error; }];
}
void dgl_notifications_show(const char *identifier, const char *title, const char *body) {
    UNMutableNotificationContent *content = [UNMutableNotificationContent new];
    content.title = [NSString stringWithUTF8String:title];
    content.body = [NSString stringWithUTF8String:body];
    content.sound = UNNotificationSound.defaultSound;
    UNNotificationRequest *request = [UNNotificationRequest requestWithIdentifier:
        [NSString stringWithUTF8String:identifier] content:content trigger:nil];
    [UNUserNotificationCenter.currentNotificationCenter addNotificationRequest:request withCompletionHandler:^(NSError *error) { (void)error; }];
}
