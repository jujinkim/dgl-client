#import <Foundation/Foundation.h>
#import <CoreText/CoreText.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

// Printable ASCII must come from the face itself, every glyph with one advance.
static BOOL dgl_face_is_monospace(CTFontDescriptorRef descriptor) {
    enum { COUNT = 0x7e - 0x21 + 1 };
    UniChar characters[COUNT];
    for (int i = 0; i < COUNT; i++) characters[i] = (UniChar)(0x21 + i);
    CTFontRef font = CTFontCreateWithFontDescriptor(descriptor, 12.0, NULL);
    if (!font) return NO;
    CGGlyph glyphs[COUNT];
    BOOL monospace = CTFontGetGlyphsForCharacters(font, characters, glyphs, COUNT);
    if (monospace) {
        CGSize advances[COUNT];
        CTFontGetAdvancesForGlyphs(font, kCTFontOrientationHorizontal, glyphs, advances, COUNT);
        double width = advances[0].width;
        monospace = width > 0;
        for (int i = 1; monospace && i < COUNT; i++) {
            if (fabs(advances[i].width - width) > width * 0.01) monospace = NO;
        }
    }
    CFRelease(font);
    return monospace;
}

/// Newline-separated UTF-8 family names. Release with `dgl_terminal_fonts_free`.
/// Reads each installed face's descriptor once instead of matching every family name.
char *dgl_terminal_fonts_copy(void) {
    @autoreleasepool {
        CTFontCollectionRef collection = CTFontCollectionCreateFromAvailableFonts(NULL);
        if (!collection) return strdup("");
        NSArray *descriptors = CFBridgingRelease(CTFontCollectionCreateMatchingFontDescriptors(collection));
        CFRelease(collection);
        NSMutableSet<NSString *> *checked = [NSMutableSet new];
        NSMutableArray<NSString *> *result = [NSMutableArray new];
        for (id item in descriptors) {
            CTFontDescriptorRef descriptor = (__bridge CTFontDescriptorRef)item;
            id family = CFBridgingRelease(CTFontDescriptorCopyAttribute(descriptor, kCTFontFamilyNameAttribute));
            if (![family isKindOfClass:[NSString class]]) continue;
            NSString *name = family;
            // Dot-prefixed families are private system UI faces.
            if (name.length == 0 || [name hasPrefix:@"."] || [name containsString:@"\n"] || [checked containsObject:name]) continue;
            [checked addObject:name];
            if (dgl_face_is_monospace(descriptor)) [result addObject:name];
        }
        const char *joined = [result componentsJoinedByString:@"\n"].UTF8String;
        return strdup(joined ? joined : "");
    }
}

void dgl_terminal_fonts_free(char *names) {
    free(names);
}
