# DGL is open source: shrink and optimize, but keep class names readable so
# crash stacks need no mapping and APK verifiers can find the Rust engine classes.
-dontobfuscate

# JNA ships no consumer rules and resolves Structure fields, Callback methods
# and native library interfaces by reflection.
-dontwarn java.awt.**
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }

# UniFFI bindings generated from dgl-uniffi; JNA binds their members by name.
-keep class uniffi.** { *; }

# scripts/collect_rust_android.py checks these Rust engine entry classes by name.
-keep class kim.jujin.dglc.RustClientConnection
-keep class kim.jujin.dglc.RustKeyTools
