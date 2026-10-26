#import <AppKit/AppKit.h>

@interface DGLTooltipOwner : NSObject <NSViewToolTipOwner>
@property(nonatomic, weak) NSView *view;
@property(nonatomic, copy) NSString *text;
@property(nonatomic) NSToolTipTag tag;
@end
@implementation DGLTooltipOwner
- (NSString *)view:(NSView *)view stringForToolTip:(NSToolTipTag)tag point:(NSPoint)point userData:(void *)data {
    return self.text;
}
- (void)dealloc {
    if (_tag) [_view removeToolTip:_tag];
}
@end

void *dgl_tooltip_create(void *view) {
    DGLTooltipOwner *owner = [DGLTooltipOwner new];
    owner.view = (__bridge NSView *)view;
    return (__bridge_retained void *)owner;
}
void dgl_tooltip_set(void *manager, double x, double y, double w, double h, const char *text) {
    DGLTooltipOwner *owner = (__bridge DGLTooltipOwner *)manager;
    if (owner.tag) [owner.view removeToolTip:owner.tag];
    owner.tag = 0;
    owner.text = text ? [NSString stringWithUTF8String:text] : nil;
    if (!owner.text || !owner.view) return;
    NSView *view = owner.view;
    NSRect rect = NSMakeRect(NSMinX(view.bounds) + x,
        view.isFlipped ? NSMinY(view.bounds) + y : NSMaxY(view.bounds) - y - h, w, h);
    owner.tag = [view addToolTipRect:rect owner:owner userData:NULL];
}
void dgl_tooltip_destroy(void *manager) {
    DGLTooltipOwner *owner = (__bridge_transfer DGLTooltipOwner *)manager;
    if (owner.tag) [owner.view removeToolTip:owner.tag];
    owner.tag = 0;
}
