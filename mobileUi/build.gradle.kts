plugins { id("com.android.library"); kotlin("plugin.compose"); kotlin("plugin.serialization") }
android {
    namespace = "kim.jujin.dgl.mobile"
    compileSdk = 36
    defaultConfig { minSdk = 34 }
    buildFeatures { compose = true }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies {
    implementation(project(":app"))
    implementation(project(":coreBindings"))
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.9.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
    implementation("org.jetbrains.compose.runtime:runtime:1.9.3")
}
dependencyLocking { lockAllConfigurations(); lockMode.set(LockMode.LENIENT) }
