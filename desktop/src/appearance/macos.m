#import <AppKit/AppKit.h>

void dgl_system_language(char *buffer, size_t length) {
    @autoreleasepool {
        NSString *tag = [NSLocale preferredLanguages].firstObject ?: @"en";
        if (![tag getCString:buffer maxLength:length encoding:NSUTF8StringEncoding] && length > 0) {
            buffer[0] = '\0';
        }
    }
}
#include <stdint.h>
#include <math.h>

@interface DGLAppearance : NSObject
@property(nonatomic) BOOL dirty;
@property(nonatomic, copy) NSArray *observers;
@end
@implementation DGLAppearance
- (void)dealloc {
    for (id token in self.observers) [[NSNotificationCenter defaultCenter] removeObserver:token];
}
@end

void *dgl_appearance_create(void) {
    DGLAppearance *state = [DGLAppearance new];
    state.dirty = YES;
    __weak DGLAppearance *weakState = state;
    NSMutableArray *tokens = [NSMutableArray new];
    for (NSNotificationName name in @[NSSystemColorsDidChangeNotification, NSApplicationDidBecomeActiveNotification]) {
        [tokens addObject:[[NSNotificationCenter defaultCenter] addObserverForName:name object:nil
            queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) { (void)note; weakState.dirty = YES; }]];
    }
    state.observers = tokens;
    return (__bridge_retained void *)state;
}
bool dgl_appearance_dirty(void *ptr) {
    DGLAppearance *state = (__bridge DGLAppearance *)ptr;
    BOOL dirty = state.dirty;
    state.dirty = NO;
    return dirty;
}
void dgl_appearance_destroy(void *ptr) { (void)CFBridgingRelease(ptr); }

bool dgl_appearance_system_dark(void) {
    NSAppearanceName match = [NSApp.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    return [match isEqualToString:NSAppearanceNameDarkAqua];
}
int64_t dgl_appearance_update(void *viewPtr, bool dark, bool system, uint32_t background) {
    @autoreleasepool {
        NSWindow *window = [(__bridge NSView *)viewPtr window];
        if (!window) return -1;
        NSAppearance *appearance = [NSAppearance appearanceNamed:dark ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
        window.appearance = system ? nil : appearance;
        // Keep the native title, traffic lights, frame and content layout rect.
        // No FullSizeContentView: AppKit reserves the actual titlebar height,
        // including its fullscreen transition, without a hardcoded spacer.
        window.titlebarAppearsTransparent = YES;
        window.backgroundColor = [NSColor colorWithSRGBRed:((background >> 16) & 255) / 255.0
            green:((background >> 8) & 255) / 255.0 blue:(background & 255) / 255.0 alpha:1];
        __block int64_t color = -1;
        [appearance performAsCurrentDrawingAppearance:^{
            NSColor *accent = [NSColor.controlAccentColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            if (accent) {
                uint32_t r = lround(fmax(0, fmin(1, accent.redComponent)) * 255);
                uint32_t g = lround(fmax(0, fmin(1, accent.greenComponent)) * 255);
                uint32_t b = lround(fmax(0, fmin(1, accent.blueComponent)) * 255);
                color = (r << 16) | (g << 8) | b;
            }
        }];
        return color;
    }
}
