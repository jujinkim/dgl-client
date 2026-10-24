# Third-party notices

This project uses unmodified third-party libraries. Version and checksum records are in the selected Gradle dependency inventory, dependency lock files and `web/package-lock.json`. `licenses/dependencies.json` maps each selected runtime package/version and artifact hash to its versioned POM (including parent POMs), license text, embedded notices and source URLs. `licenses/native-components.json` records native source revisions and notices.

| Component | Version | License / source |
|---|---|---|
| Kotlin, Compose, AndroidX | pinned in Gradle | Apache-2.0; https://github.com/JetBrains/kotlin ; https://github.com/JetBrains/compose-multiplatform ; https://android.googlesource.com/platform/frameworks/support/ |
| SSHJ (reference tests only) | 0.40.0 | Apache-2.0; https://github.com/hierynomus/sshj |
| Bouncy Castle (tests/build tools only) | 1.86 | Bouncy Castle licence (MIT-style); https://www.bouncycastle.org/licence.html |
| JediTerm (JVM reference tests only) | 3.59 | Artifact POM declares LGPL-3.0; https://github.com/JetBrains/jediterm |
| xterm.js / addon-fit | 6.0.0 / 0.11.0 | MIT; bundled LICENSE-xterm and LICENSE-fit in Android assets; https://github.com/xtermjs/xterm.js |
| JNA | 5.18.1 | LGPL-2.1-or-later or Apache-2.0; https://github.com/java-native-access/jna |
| SLF4J (reference tests only) | 2.0.17 | MIT; https://www.slf4j.org/license.html |
| Apache MINA SSHD | 2.20.0 (tests only) | Apache-2.0; https://github.com/apache/mina-sshd |

The current JediTerm upstream README offers Apache-2.0/LGPL-3.0 dual licensing, but the selected 3.59 artifact declares LGPL-3.0. Treat the selected artifact as LGPL-3.0 until its release licensing is independently confirmed. The previous JVM application and its installer tasks have been removed. JediTerm remains in
reference rendering tests only; Android and the GPUI application do not distribute it.
The original library was not shaded or modified. No restriction on reverse engineering
for debugging modifications to those libraries is imposed by this project.

Corresponding library sources:

- https://packages.jetbrains.team/maven/p/ij/intellij-dependencies/org/jetbrains/jediterm/jediterm-core/3.59/jediterm-core-3.59-sources.jar
- https://packages.jetbrains.team/maven/p/ij/intellij-dependencies/org/jetbrains/jediterm/jediterm-ui/3.59/jediterm-ui-3.59-sources.jar

`prepareThirdPartyNotices` copies these notices, reviewed `licenses/` files, both verified 3.59 source JARs, and the packaging JDK's complete `legal/` tree and `release` identity into historical JVM verification resources. `SHA256SUMS` enumerates every delivered file. Generated build/cache files are excluded. Android assets include the notices and license evidence; xterm notices are bundled separately. The Android app does not distribute JediTerm or a JVM.

The tested replacement build recipe is `licenses/jediterm-rebuild/README.md`. It builds both published 3.59 source JARs using JDK 17, Kotlin 2.4.21 and Gradle 9.7.0, preserving upstream Kotlin module names. It is not a byte-identical upstream build. The library JARs are replaceable; the original app does not shade or modify them.

Skiko 0.9.22.2 uses Skia revision `a00c390e98`, package `m132-a00c390e98-1`. Versioned Skiko LICENSE/NOTICE, Skia LICENSE/DEPS, archive notices and linked native component notices are included. Portions of this software are copyright the FreeType Project (https://www.freetype.org). JNA 5.18.1 native dispatch and its embedded libffi license are also included. Native notices may conservatively cover additional components present in the upstream Skia archive.

The JVM notice bundle identifies the actual packaging JDK; the local Homebrew JDK notice bundle is development evidence, not proof of a packaged Windows Temurin runtime. Public Windows distribution must retain and verify the Temurin runtime's own legal files and exact source/release identity. The previous Windows packaging path is retired; retained source/replacement tooling is historical test evidence. No repository-wide license for new project code is inferred on the owner's behalf.


## Rust candidate dependencies (2026-10-24)

The Rust Android application and Mac GPUI candidate use the versions and checksums in `Cargo.lock`.
`scripts/collect_rust_notices.py` bundles registry-provided license texts, Cargo manifests,
and `DEPENDENCIES.json` with each candidate (Mac Resources / Android assets).
The inventory includes build and target-specific dependencies; entries without an upstream
license text explicitly have an empty `licenseFiles` map. This is not a completed public
release license audit. The GPUI-derived text input additionally includes
`desktop/NOTICE` and `desktop/LICENSE-APACHE-GPUI`.
