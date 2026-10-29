import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    kotlin("plugin.compose")
}

android {
    namespace = "kim.jujin.dglc"
    compileSdk = 36
    buildToolsVersion = "36.0.0"
    defaultConfig {
        applicationId = "kim.jujin.dglc"
        minSdk = 34
        targetSdk = 36
        versionCode = 1
        versionName = rootProject.extra["appVersion"] as String
        ndk { abiFilters += setOf("arm64-v8a", "armeabi-v7a", "x86", "x86_64") }
        testInstrumentationRunner = if (providers.gradleProperty("dglResourceProbe").orNull == "true")
            "kim.jujin.dglc.ResourceInstrumentation" else "androidx.test.runner.AndroidJUnitRunner"
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    buildFeatures { compose = true; buildConfig = true }
    sourceSets.getByName("androidTest").kotlin.srcDir("../app/src/referenceTest/kotlin")
    packaging {
        resources.excludes += setOf("META-INF/versions/9/OSGI-INF/MANIFEST.MF", "META-INF/INDEX.LIST", "META-INF/DEPENDENCIES")
        resources.merges += setOf("META-INF/LICENSE.md", "META-INF/LICENSE", "META-INF/NOTICE.md", "META-INF/NOTICE")
    }
    buildTypes { release { isMinifyEnabled = false } }
}
kotlin { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }
dependencies {
    implementation(project(":app"))
    implementation(project(":mobileUi"))
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
    implementation("org.jetbrains.compose.runtime:runtime:1.9.3")
    androidTestImplementation("com.hierynomus:sshj:0.40.0")
    androidTestImplementation("org.bouncycastle:bcprov-jdk18on:1.86")
    androidTestImplementation("org.bouncycastle:bcpkix-jdk18on:1.86")
    androidTestImplementation("org.slf4j:slf4j-nop:2.0.17")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:core:1.6.1")
    androidTestImplementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
}
dependencyLocking { lockAllConfigurations(); lockMode.set(LockMode.LENIENT) }
abstract class NoticeAssets : Sync() {
    @get:OutputDirectory abstract val assetsDirectory: DirectoryProperty
}
val prepareAndroidNotices by tasks.registering(NoticeAssets::class) {
    assetsDirectory.set(layout.buildDirectory.dir("generated/noticeAssets"))
    from(rootProject.file("THIRD_PARTY_NOTICES.md"))
    from(rootProject.file("licenses")) {
        into("licenses")
        exclude("**/build/**", "**/.gradle/**", "**/.kotlin/**")
    }
    into(assetsDirectory.dir("third-party"))
}
androidComponents.onVariants { variant ->
    variant.sources.assets?.addGeneratedSourceDirectory(prepareAndroidNotices) { it.assetsDirectory }
}
tasks.matching { it.name.matches(Regex("merge.*Assets")) || it.name.contains("lint", ignoreCase = true) }
    .configureEach { dependsOn(prepareAndroidNotices) }
