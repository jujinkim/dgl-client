import org.jetbrains.compose.desktop.application.dsl.TargetFormat
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import java.security.MessageDigest

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
            dependencies {
                implementation("com.hierynomus:sshj:0.40.0")
                implementation("org.bouncycastle:bcprov-jdk18on:1.86")
                implementation("org.bouncycastle:bcpkix-jdk18on:1.86")
                implementation("org.slf4j:slf4j-nop:2.0.17")
            }
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
            dependencies {
                implementation(kotlin("test-junit"))
                implementation("org.apache.sshd:sshd-core:2.20.0")
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "kim.jujin.rovessh.MainKt"
        nativeDistributions {
            targetFormats(TargetFormat.Msi, TargetFormat.Exe)
            packageName = "dgl-client"
            packageVersion = "0.1.0"
            description = "SSH and tmux terminal client"
            vendor = "dgl client"
            modules("java.naming", "jdk.crypto.ec", "jdk.unsupported")
            appResourcesRootDir.set(layout.buildDirectory.dir("generated/distributionResources"))
            windows {
                menuGroup = "dgl client"
                shortcut = true
                upgradeUuid = "db8ee957-3c5c-494d-a786-3daf2dbf57d9"
            }
        }
    }
}

tasks.withType<Test>().configureEach {
    testLogging { events("passed", "skipped", "failed"); showStandardStreams = false }
    maxHeapSize = "1g"
}

// Kept outside the production distribution. Used only by isolated Windows CI accounts.
val desktopTests = kotlin.targets.getByName("desktop").compilations.getByName("test")
tasks.register<Sync>("prepareDpapiProbe") {
    dependsOn("desktopTestClasses")
    from(desktopTests.output.allOutputs)
    from(desktopTests.runtimeDependencyFiles)
    into(layout.buildDirectory.dir("dpapi-probe"))
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}

dependencyLocking {
    lockAllConfigurations()
    // Different desktop targets add their own native Skiko artifacts; common versions stay locked.
    lockMode.set(LockMode.LENIENT)
}
val librarySources by configurations.creating { isCanBeConsumed = false; isTransitive = false }
dependencies {
    add(librarySources.name, "org.jetbrains.jediterm:jediterm-core:3.59:sources")
    add(librarySources.name, "org.jetbrains.jediterm:jediterm-ui:3.59:sources")
}
val prepareThirdPartyNotices by tasks.registering {
    val destination = layout.buildDirectory.dir("generated/distributionResources/common/third-party")
    inputs.file(rootProject.file("THIRD_PARTY_NOTICES.md"))
    val licenseFiles = rootProject.fileTree("licenses") { exclude("**/build/**", "**/.gradle/**", "**/.kotlin/**") }
    inputs.files(licenseFiles)
    val javaHome = file(System.getProperty("java.home"))
    inputs.dir(javaHome.resolve("legal"))
    inputs.file(javaHome.resolve("release"))
    inputs.files(librarySources)
    val windowsBuild = System.getProperty("os.name").startsWith("Windows")
    val temurinSource = layout.buildDirectory.dir("temurin-source")
    if (windowsBuild) inputs.dir(temurinSource)
    outputs.dir(destination)
    doLast {
        val directory = destination.get().asFile
        directory.deleteRecursively(); directory.mkdirs()
        rootProject.file("THIRD_PARTY_NOTICES.md").copyTo(directory.resolve("THIRD_PARTY_NOTICES.md"))
        project.copy { from(licenseFiles); into(directory.resolve("licenses")) }
        librarySources.files.forEach { it.copyTo(directory.resolve(it.name)) }
        javaHome.resolve("legal").copyRecursively(directory.resolve("jvm/legal"))
        javaHome.resolve("release").copyTo(directory.resolve("jvm/release"))
        if (windowsBuild) {
            check(temurinSource.get().file("source.json").asFile.isFile) { "Run scripts/prepare_temurin_source.py before packaging" }
            temurinSource.get().asFile.copyRecursively(directory.resolve("jvm/source"))
            javaHome.resolve("lib/src.zip").copyTo(directory.resolve("jvm/src.zip"))
        }
        directory.resolve("SHA256SUMS").writeText(directory.walkTopDown().filter { it.isFile }.sortedBy { it.relativeTo(directory).path }.joinToString("\n", postfix = "\n") {
            val sha = MessageDigest.getInstance("SHA-256").digest(it.readBytes()).joinToString("") { byte -> "%02x".format(byte) }
            "$sha  ${it.relativeTo(directory).invariantSeparatorsPath}"
        })
    }
}
tasks.matching { it.name == "createDistributable" || it.name == "prepareAppResources" || it.name.startsWith("packageMsi") || it.name.startsWith("packageExe") }.configureEach {
    dependsOn(prepareThirdPartyNotices)
}
