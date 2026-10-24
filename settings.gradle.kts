pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement {
    repositories {
        google(); mavenCentral()
        maven("https://packages.jetbrains.team/maven/p/ij/intellij-dependencies") {
            content { includeGroup("org.jetbrains.jediterm") }
        }
    }
}
rootProject.name = "dgl-client"
include(":app", ":androidApp", ":coreBindings", ":mobileUi")
require(!providers.gradleProperty("dglEngine").isPresent) {
    "dglEngine was removed. Android uses the Rust engine; omit -PdglEngine."
}
