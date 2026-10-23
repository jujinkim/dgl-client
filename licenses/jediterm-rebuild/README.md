# JediTerm 3.59 sources, rebuilding and replacement

The adjacent source JARs contain the complete sources published with the selected
3.59 core/UI artifacts. This directory supplies a minimal replacement build using
JDK 17, Gradle 9.7.0 and Kotlin 2.4.21. It is a project-supplied build recipe, not an
upstream build script or a claim of byte-identical reproduction. Both libraries
remain separate, unmodified JARs in the original desktop application.

From a source checkout with its verified Gradle Wrapper, run:

```sh
./gradlew -p licenses/jediterm-rebuild jar --no-build-cache \
  -PsourcesDir="$PWD/app/build/generated/distributionResources/common/third-party"
```

From an extracted third-party bundle, use a checksum-verified Gradle 9.7.0
installation and run `gradle -p licenses/jediterm-rebuild jar --no-build-cache`.
The source JARs must be at the bundle root. Initial builds need Maven Central
and the Gradle plugin portal for the pinned compiler dependencies.

The recipe unpacks sources into each module's `build/upstreamSources`. To modify
sources, copy that directory to a persistent directory and change the main Java
and Kotlin source directories in `build.gradle.kts` to point there; remove the
unpack task dependency so that subsequent builds preserve your edits.

Output: `jediterm-core/build/libs/jediterm-core-3.59.jar` and
`jediterm-ui/build/libs/jediterm-ui-3.59.jar`. Close the desktop app and replace its
two corresponding JARs under the installation's `app` directory. Keep the file
names unchanged. Alternatively, change the main project's two `jediterm-*`
dependencies to `files(...)` pointing to the rebuilt JARs, then rebuild with
`:app:packageMsi :app:packageExe` on Windows. Retain the runtime SLF4J and Kotlin
libraries and all license notices. Back up the original installation first.
The project imposes no restriction on reverse engineering to debug changes to
these LGPL libraries. Application code is not relicensed by this notice.

License: the selected 3.59 POM declares LGPL 3.0. The bundled LGPL and GPL texts
apply; a later upstream dual-license statement is not applied retroactively.
Exact source URLs and SHA-256 values are recorded in the delivery manifest and
Gradle verification metadata. Windows installation/replacement must be validated
on Windows before a public binary release.
