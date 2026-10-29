plugins { id("com.android.library") }
android {
    namespace = "kim.jujin.dglc.bindings"
    compileSdk = 36
    defaultConfig { minSdk = 34; ndk { abiFilters += setOf("arm64-v8a", "armeabi-v7a", "x86", "x86_64") } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies {
    implementation("androidx.annotation:annotation:1.9.1")
    api("net.java.dev.jna:jna:5.18.1@aar")
    api("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
}
val buildRustBindings by tasks.registering(Exec::class) {
    workingDir(rootProject.projectDir)
    commandLine("python3", "scripts/build_rust_android.py")
    inputs.files(rootProject.file("scripts/build_rust_android.py"), rootProject.fileTree("crates"), rootProject.fileTree("locales"), rootProject.file("Cargo.toml"), rootProject.file("Cargo.lock"))
    outputs.dir("src/main/jniLibs")
    outputs.dir("src/main/kotlin/uniffi")
}
tasks.named("preBuild") { dependsOn(buildRustBindings) }
abstract class RustNotices : Exec() {
    @get:OutputDirectory abstract val assetsDirectory: DirectoryProperty
}
val prepareRustNotices by tasks.registering(RustNotices::class) {
    assetsDirectory.set(layout.buildDirectory.dir("generated/rustNotices"))
    workingDir(rootProject.projectDir)
    commandLine("python3", "scripts/collect_rust_notices.py", "--package", "dgl-uniffi", "--output", "coreBindings/build/generated/rustNotices/third-party-rust")
    inputs.file(rootProject.file("Cargo.lock"))
    inputs.file(rootProject.file("scripts/collect_rust_notices.py"))
    inputs.file(rootProject.file("scripts/build_rust_android.py")) // default Android target triples
    outputs.dir(layout.buildDirectory.dir("generated/rustNotices"))
}
tasks.named("preBuild") { dependsOn(prepareRustNotices) }
dependencyLocking { lockAllConfigurations(); lockMode.set(LockMode.LENIENT) }


androidComponents.onVariants { variant ->
    variant.sources.assets?.addGeneratedSourceDirectory(prepareRustNotices) { it.assetsDirectory }
}
