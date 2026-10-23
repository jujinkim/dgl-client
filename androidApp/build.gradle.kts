import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    kotlin("plugin.compose")
}

android {
    namespace = "kim.jujin.rovessh"
    compileSdk = 36
    buildToolsVersion = "36.0.0"
    defaultConfig {
        applicationId = "kim.jujin.rovessh"
        minSdk = 34
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = if (providers.gradleProperty("dglResourceProbe").orNull == "true")
            "kim.jujin.rovessh.ResourceInstrumentation" else "androidx.test.runner.AndroidJUnitRunner"
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    buildFeatures { compose = true }
    sourceSets.getByName("androidTest").kotlin.srcDir("../app/src/resourceProbe/kotlin")
    packaging {
        resources.excludes += setOf("META-INF/versions/9/OSGI-INF/MANIFEST.MF", "META-INF/INDEX.LIST", "META-INF/DEPENDENCIES")
        resources.merges += setOf("META-INF/LICENSE.md", "META-INF/LICENSE", "META-INF/NOTICE.md", "META-INF/NOTICE")
    }
    buildTypes { release { isMinifyEnabled = false } }
}
kotlin { compilerOptions { jvmTarget.set(JvmTarget.JVM_17) } }
dependencies {
    implementation(project(":app"))
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.4")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
    implementation("org.jetbrains.compose.runtime:runtime:1.9.3")
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
