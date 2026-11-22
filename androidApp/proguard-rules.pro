# Release keeps R8 shrinking, optimization and obfuscation on: Google Play requires
# at least 25% of each. The AAB bundles mapping.txt (BUNDLE-METADATA) so Play Console
# retraces crash stacks; keep androidApp/build/outputs/mapping/release/mapping.txt
# for retracing logs collected elsewhere.
#
# Persisted JSON (settings, workspace layout, credentials) uses kotlinx.serialization
# names compiled in as string literals, so renaming classes does not change it.
# Never add -adaptclassstrings: it could rewrite those stored type names.

# JNA ships no consumer rules and resolves Structure fields, Callback methods
# and native library interfaces by reflection.
-dontwarn java.awt.**
-keep class com.sun.jna.** { *; }
-keep class * extends com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }

# UniFFI bindings generated from dgl-uniffi; JNA binds their members by name.
-keep class uniffi.** { *; }

# verify_rust_payload (scripts/collect_rust_android.py) checks these names in the debug
# and, via scripts/collect_artifacts.py, the release APK.
-keep class kim.jujin.dglc.RustClientConnection
-keep class kim.jujin.dglc.RustKeyTools
