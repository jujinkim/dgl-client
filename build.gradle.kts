buildscript {
    // Security fixes for AGP's build-only dependencies; never rewrite application graphs.
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
        resolutionStrategy.eachDependency {
            when {
                requested.group == "org.bouncycastle" && requested.name in setOf("bcprov-jdk18on", "bcpkix-jdk18on", "bcutil-jdk18on") -> useVersion("1.86")
                requested.group == "org.bitbucket.b_c" && requested.name == "jose4j" -> useVersion("0.9.7")
                requested.group == "org.jdom" && requested.name == "jdom2" -> useVersion("2.0.6.1")
                requested.group == "org.apache.commons" && requested.name == "commons-lang3" -> useVersion("3.18.0")
            }
        }
    }
}

plugins {
    kotlin("multiplatform") version "2.4.21" apply false
    kotlin("plugin.compose") version "2.4.21" apply false
    kotlin("plugin.serialization") version "2.4.21" apply false
    id("org.jetbrains.compose") version "1.9.3" apply false
    id("com.android.kotlin.multiplatform.library") version "9.3.3" apply false
    id("com.android.application") version "9.3.3" apply false
    id("com.android.library") version "9.3.3" apply false
}

apply(from = "gradle/dependency-inventory.gradle")

// Cargo's workspace version is the source for every product surface and package.
val workspaceManifest = providers.fileContents(layout.projectDirectory.file("Cargo.toml")).asText.get()
val workspacePackage = workspaceManifest.substringAfter("[workspace.package]").substringBefore("\n[")
val appVersion = Regex("""(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"\s*$""")
    .find(workspacePackage)?.groupValues?.get(1)
    ?: error("Cargo.toml must declare a numeric workspace.package.version")
extra["appVersion"] = appVersion
val (major, minor, patch) = appVersion.split('.').map { it.toInt() }
require(major in 0..9 && minor in 0..99 && patch in 0..99) {
    "Android version requires major 0..9, minor 0..99 and patch 0..99"
}
val androidVersionCode = major * 10000 + minor * 100 + patch
require(androidVersionCode > 0) { "Android versionCode must be positive" }
extra["androidVersionCode"] = androidVersionCode

subprojects {
    // Lint runs in a separate classloader and does not inherit the plugin classpath fixes.
    configurations.matching { it.name == "androidLintTool" }.configureEach {
        resolutionStrategy.eachDependency {
            when {
                requested.group == "org.bouncycastle" && requested.name in setOf("bcprov-jdk18on", "bcpkix-jdk18on", "bcutil-jdk18on") -> useVersion("1.86")
                requested.group == "org.apache.commons" && requested.name == "commons-lang3" -> useVersion("3.18.0")
                requested.group == "org.apache.httpcomponents" && requested.name == "httpclient" -> useVersion("4.5.14")
            }
        }
    }
}
