# Packaged JVM notices

Desktop packaging copies the actual build JDK's complete `legal/` directory and
`release` file to `third-party/jvm/`. The bundle's `SHA256SUMS` includes their
contents. Java modules selected by jlink retain their own legal files in the
runtime image as well. Android does not redistribute this JVM.

The local validation host uses Homebrew OpenJDK 17.0.20.1+0 on macOS ARM64. Its
notices are development evidence; no macOS or Windows app image was produced in
this P4-03 verification. Windows CI selects Eclipse Temurin JDK 17. At release,
inspect the packaged runtime's exact version, vendor, source revision and legal
files, and preserve the corresponding vendor source download alongside the
release record. Do not substitute this local Homebrew identity for Temurin.

Upstream sources and distribution information:

- OpenJDK 17 updates: https://github.com/openjdk/jdk17u
- Temurin source/releases: https://github.com/adoptium/temurin17-binaries/releases
- Homebrew build/source metadata: https://github.com/Homebrew/homebrew-core/tree/master/Formula/o

These pointers identify vendor source channels; they are not an assertion that a
particular unbuilt Windows distribution has been inspected. Public release stays
blocked until its actual runtime/source identity and legal files are verified.
