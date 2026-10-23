import org.jetbrains.kotlin.gradle.dsl.KotlinJvmProjectExtension
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins { kotlin("jvm") version "2.4.21" apply false }

subprojects {
    apply(plugin = "org.jetbrains.kotlin.jvm")
    apply(plugin = "java-library")
    group = "org.jetbrains.jediterm"
    version = "3.59"
    val sources = providers.gradleProperty("sourcesDir").orElse(rootProject.file("../..").absolutePath)
    val unpackSources = tasks.register<Sync>("unpackSources") {
        from(zipTree(file(sources.get()).resolve("${project.name}-3.59-sources.jar")))
        exclude("META-INF/**")
        into(layout.buildDirectory.dir("upstreamSources"))
    }
    configure<KotlinJvmProjectExtension> {
        jvmToolchain(17)
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_17)
            moduleName.set(project.name.removePrefix("jediterm-"))
        }
        sourceSets.named("main") { kotlin.srcDir(unpackSources.map { it.destinationDir }) }
    }
    configure<JavaPluginExtension> {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
        sourceSets.named("main") {
            java.srcDir(unpackSources.map { it.destinationDir })
            resources.srcDir(unpackSources.map { it.destinationDir })
            resources.exclude("**/*.java", "**/*.kt")
        }
    }
    dependencies {
        add("implementation", "org.slf4j:slf4j-api:2.0.17")
        add("compileOnly", "org.jetbrains:annotations:24.0.1")
        if (project.name == "jediterm-ui") add("api", project(":jediterm-core"))
    }
    dependencyLocking { lockAllConfigurations() }
}
