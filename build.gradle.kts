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
}

apply(from = "gradle/dependency-inventory.gradle")

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
