#import <AppKit/AppKit.h>
#import <objc/runtime.h>

extern void dgl_file_retain(const void *);
extern void dgl_file_release(const void *);
extern void dgl_file_write(const void *, size_t, const char *, uintptr_t);

@interface DglPromise : NSObject <NSFilePromiseProviderDelegate, NSDraggingSource>
@property(copy) NSString *name;
@property const void *context;
@property size_t index;
@end
@implementation DglPromise
- (void)dealloc { if (_context) dgl_file_release(_context); }
- (NSString *)filePromiseProvider:(NSFilePromiseProvider *)provider fileNameForType:(NSString *)type { return _name; }
- (void)filePromiseProvider:(NSFilePromiseProvider *)provider writePromiseToURL:(NSURL *)url completionHandler:(void (^)(NSError *))done {
    dgl_file_write(_context, _index, url.fileSystemRepresentation, (uintptr_t)CFBridgingRetain([done copy]));
}
- (NSDragOperation)draggingSession:(NSDraggingSession *)session sourceOperationMaskForDraggingContext:(NSDraggingContext)context { return NSDragOperationCopy; }
@end

bool dgl_file_drag(void *viewPtr, const char **names, const bool *dirs, size_t count, const void *context) {
    NSView *view = (__bridge NSView *)viewPtr;
    NSEvent *event = NSApp.currentEvent;
    if (!event || !count || !view.window || event.window != view.window) return false;
    NSMutableArray *items = [NSMutableArray arrayWithCapacity:count];
    DglPromise *source = nil;
    NSPoint point = [view convertPoint:event.locationInWindow fromView:nil];
    for (size_t i = 0; i < count; i++) {
        DglPromise *delegate = [DglPromise new];
        delegate.name = [NSString stringWithUTF8String:names[i]];
        delegate.index = i;
        dgl_file_retain(context);
        delegate.context = context;
        if (!source) source = delegate;
        NSFilePromiseProvider *provider = [[NSFilePromiseProvider alloc] initWithFileType:dirs[i] ? @"public.folder" : @"public.data" delegate:delegate];
        provider.userInfo = delegate; // The provider's delegate property is weak.
        NSDraggingItem *item = [[NSDraggingItem alloc] initWithPasteboardWriter:provider];
        NSImage *image = [NSImage imageWithSystemSymbolName:dirs[i] ? @"folder" : @"doc" accessibilityDescription:delegate.name];
        [item setDraggingFrame:NSMakeRect(point.x, point.y, 32, 32) contents:image];
        [items addObject:item];
    }
    NSDraggingSession *session = [view beginDraggingSessionWithItems:items event:event source:source];
    session.draggingFormation = NSDraggingFormationStack;
    return session != nil;
}

void dgl_file_written(uintptr_t completion, const char *error) {
    void (^done)(NSError *) = CFBridgingRelease((void *)completion);
    NSString *reason = error ? [NSString stringWithUTF8String:error] : nil;
    dispatch_async(dispatch_get_main_queue(), ^{
        done(reason ? [NSError errorWithDomain:@"DGL.SFTP" code:1 userInfo:@{NSLocalizedDescriptionKey:reason}] : nil);
    });
}
