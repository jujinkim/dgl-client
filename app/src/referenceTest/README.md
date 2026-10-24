# Previous engine regression reference

This source set is compiled only into JVM tests and the Android instrumentation
test APK. It preserves existing fixtures, SSH fixture administration and engine
regression evidence. It is not an application engine or a runtime fallback.

The production Android APK uses `RustClientConnection` and `RustKeyTools` through
UniFFI. SSHJ and Bouncy Castle are test dependencies only. The desktop application
entry point is the GPUI executable in `desktop/`.
