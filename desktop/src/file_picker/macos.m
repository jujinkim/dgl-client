#import <AppKit/AppKit.h>

void dgl_select_private_key(const char *directory, void *context,
                           void (*complete)(void *, const char *)) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.title = @"개인키 파일 선택";
    panel.prompt = @"가져오기";
    panel.canChooseFiles = YES;
    panel.canChooseDirectories = NO;
    panel.allowsMultipleSelection = NO;
    panel.showsHiddenFiles = YES;
    if (directory) {
        panel.directoryURL = [NSURL fileURLWithPath:[NSString stringWithUTF8String:directory]
                                       isDirectory:YES];
    }
    void (^completion)(NSModalResponse) = ^(NSModalResponse response) {
        complete(context, response == NSModalResponseOK ? panel.URL.path.UTF8String : NULL);
    };
    if (NSApp.keyWindow) {
        [panel beginSheetModalForWindow:NSApp.keyWindow completionHandler:completion];
    } else {
        [panel beginWithCompletionHandler:completion];
    }
}
