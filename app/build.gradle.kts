import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform")
    kotlin("plugin.compose")
    kotlin("plugin.serialization")
    id("org.jetbrains.compose")
    id("com.android.kotlin.multiplatform.library")
}

kotlin {
    android {
        namespace = "kim.jujin.rovessh.shared"
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
                implementation("net.java.dev.jna:jna-platform:5.18.1")
            }
        }
        val desktopTest by getting {
            kotlin.srcDir("src/resourceProbe/kotlin")
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

// Test-only classpath for the optional resource probe; not a product distribution.
val desktopTests = kotlin.targets.getByName("desktop").compilations.getByName("test")
// A main class, never a JUnit test; the normal exact test inventory is unchanged.
tasks.register<Sync>("prepareResourceProbe") {
    dependsOn("desktopTestClasses")
    from(desktopTests.output.allOutputs)
    from(desktopTests.runtimeDependencyFiles)
    into(layout.buildDirectory.dir("resource-probe"))
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}

dependencyLocking {
    lockAllConfigurations()
    // Different desktop targets add their own native Skiko artifacts; common versions stay locked.
    lockMode.set(LockMode.LENIENT)
}
