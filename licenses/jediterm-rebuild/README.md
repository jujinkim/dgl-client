# JediTerm 3.59 reference rebuild

JediTerm is retained for JVM rendering regression tests. Android and the Rust GPUI
application do not distribute it. The previous JVM application and MSI/EXE
packaging tasks have been removed; their history is in [HISTORY.md](../../docs/HISTORY.md).

This directory supplies a replacement build using JDK 17, Gradle 9.7.0 and Kotlin
2.4.21. It is a project-supplied recipe, not an upstream build script or a claim of
byte-identical reproduction.

1. Obtain `jediterm-core-3.59-sources.jar` and `jediterm-ui-3.59-sources.jar` from the
   exact URLs in [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md).
2. Verify each file against its source artifact SHA-256 in
   `gradle/verification-metadata.xml`; keep them together in a local directory,
   for example `.tools/jediterm-sources`.
3. From the repository root, run:

```sh
./gradlew -p licenses/jediterm-rebuild jar --no-build-cache \
  -PsourcesDir="$PWD/.tools/jediterm-sources"
```

Initial builds need Maven Central and the Gradle plugin portal for the pinned
compiler dependencies. The recipe unpacks sources into each module's
`build/upstreamSources`. To modify sources, copy that directory elsewhere and
point the main Java/Kotlin source directories there; remove the unpack task
dependency so subsequent builds preserve edits.

Outputs relative to this directory: `jediterm-core/build/libs/jediterm-core-3.59.jar`
and `jediterm-ui/build/libs/jediterm-ui-3.59.jar`. To test replacements, point the
main project's two JediTerm dependencies to those JARs and run `:app:desktopTest`.
Keep dependency notices and the supporting Kotlin/SLF4J dependencies.

The selected 3.59 POM declares LGPL 3.0. The bundled LGPL and GPL texts apply; a
later upstream dual-license statement is not applied retroactively. The project
imposes no restriction on reverse engineering to debug changes to these LGPL
libraries. Application code is not relicensed by this notice.
