#import <Foundation/Foundation.h>
#include <stdbool.h>

/// macOS Low Power Mode, which the user turns on in Battery settings (macOS 12+).
bool dgl_power_saving(void) {
    if (@available(macOS 12.0, *)) {
        return [NSProcessInfo processInfo].isLowPowerModeEnabled;
    }
    return false;
}
