import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    kotlin("plugin.compose")
    kotlin("plugin.serialization")
    id("org.jetbrains.compose")
    id("com.android.kotlin.multiplatform.library")
}

val appVersion = rootProject.extra["appVersion"] as String
val versionSources = layout.buildDirectory.dir("generated/appVersion")
val generateAppVersion by tasks.registering {
    inputs.property("appVersion", appVersion)
    outputs.dir(versionSources)
    doLast {
        val file = versionSources.get().file("kim/jujin/dglc/AppVersion.kt").asFile
        file.parentFile.mkdirs()
        file.writeText("// Generated from Cargo.toml; do not edit.\npackage kim.jujin.dglc\n\nconst val APP_VERSION = \"$appVersion\"\n")
    }
}

kotlin {
    android {
        namespace = "kim.jujin.dglc.shared"
        compileSdk = 36
        minSdk = 34
        buildToolsVersion = "36.0.0"
        compilerOptions { jvmTarget.set(JvmTarget.JVM_17) }
        androidResources { enable = true }
    }
    jvm("desktop") { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }
    sourceSets {
        commonMain.dependencies {
            implementation(compose.runtime)
            implementation(compose.foundation)
            implementation(compose.material3)
            implementation(compose.ui)
            implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
            implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
        }
        commonTest.dependencies {
            implementation(kotlin("test"))
            implementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.2")
        }
        val jvmSharedMain by creating {
            dependsOn(commonMain.get())
            kotlin.srcDir(generateAppVersion)
        }
        androidMain {
            dependsOn(jvmSharedMain)
            dependencies {
                implementation("androidx.activity:activity-compose:1.10.1")
                implementation("androidx.webkit:webkit:1.14.0")
            }
        }
        val desktopMain by getting {
            dependsOn(jvmSharedMain)
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.10.2")
                implementation("org.jetbrains.jediterm:jediterm-ui:3.59")
                implementation("org.jetbrains.jediterm:jediterm-core:3.59")
                // JediTerm asks for slf4j-api 2.0.9; keep the version listed in verification-metadata.xml.
                implementation("org.slf4j:slf4j-api:2.0.17")
            }
        }
        val desktopTest by getting {
            kotlin.srcDir("src/referenceTest/kotlin")
            resources.srcDir(rootProject.file("fixtures"))
            dependencies {
                implementation(kotlin("test-junit"))
                implementation("org.apache.sshd:sshd-core:2.20.0")
                implementation("com.hierynomus:sshj:0.40.0")
                implementation("org.bouncycastle:bcprov-jdk18on:1.86")
                implementation("org.bouncycastle:bcpkix-jdk18on:1.86")
                implementation("org.slf4j:slf4j-nop:2.0.17")
            }
        }
    }
}

// JVM target remains for historical contract/rendering tests; the shipped desktop is GPUI.

tasks.withType<Test>().configureEach {
    testLogging {
        events("passed", "skipped", "failed")
        showStandardStreams = false
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
    }
    maxHeapSize = "1g"
}

dependencyLocking {
    lockAllConfigurations()
    // Different desktop targets add their own native Skiko artifacts; common versions stay locked.
    lockMode.set(LockMode.LENIENT)
}
