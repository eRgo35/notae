import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.roborazzi)
}

// The Cargo workspace this app's Rust core lives in.
val workspaceDir: File = rootDir.resolve("../..").canonicalFile
val cargoTargetDir: File =
    providers.environmentVariable("CARGO_TARGET_DIR").map(::File).getOrElse(workspaceDir.resolve("target"))
val workspaceVersion = workspaceVersion(workspaceDir.resolve("Cargo.toml").readText())

// Settings' "Build": the short commit hash, as 0.3.0 showed it, or "unknown" outside a Git
// checkout or without Git installed (a missing `git` throws rather than failing with an exit code).
val buildIdentity: String = runCatching {
    providers.exec {
        workingDir = workspaceDir
        commandLine("git", "rev-parse", "--short", "HEAD")
        isIgnoreExitValue = true
    }.standardOutput.asText.get().trim()
}.getOrDefault("").ifEmpty { "unknown" }

android {
    namespace = "com.c2yz.notae"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "com.c2yz.notae"
        minSdk = 26
        targetSdk = 36
        versionName = workspaceVersion
        versionCode = versionCodeFor(workspaceVersion)
        buildConfigField("String", "BUILD_IDENTITY", "\"$buildIdentity\"")
    }

    // Release signing comes only from these variables (docs/android-release.md); without all four,
    // assembleRelease builds an unsigned APK.
    val keystore = providers.environmentVariable("NOTAE_RELEASE_KEYSTORE").orNull
    val keystorePassword = providers.environmentVariable("NOTAE_KEYSTORE_PASSWORD").orNull
    val keyAlias = providers.environmentVariable("NOTAE_KEY_ALIAS").orNull
    val keyPassword = providers.environmentVariable("NOTAE_KEY_PASSWORD").orNull
    if (keystore != null && keystorePassword != null && keyAlias != null && keyPassword != null) {
        signingConfigs.create("release") {
            storeFile = file(keystore)
            storePassword = keystorePassword
            this.keyAlias = keyAlias
            this.keyPassword = keyPassword
        }
    }

    buildTypes {
        debug {
            // x86_64 too, for the x86_64 emulator (arm64 code aborts under its translation).
            ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
        }
        release {
            ndk { abiFilters += "arm64-v8a" }
            signingConfig = signingConfigs.findByName("release")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    // Robolectric needs the merged resources for the Compose component tests.
    testOptions.unitTests.isIncludeAndroidResources = true

    // VersionCodeTest tests the build's own versionCodeFor.
    sourceSets.getByName("test").kotlin.directories += "../buildSrc/src/main/kotlin"
}

/** Runs `cargo` with [arguments] in the workspace; Cargo itself decides what is out of date. */
abstract class CargoTask : DefaultTask() {
    @get:Input
    abstract val arguments: ListProperty<String>

    @get:Input
    abstract val environment: MapProperty<String, String>

    @get:Internal
    abstract val workspace: DirectoryProperty

    /** Where the task writes its output; unset for [arguments] that write only to Cargo's target. */
    @get:OutputDirectory
    @get:Optional
    abstract val outputDir: DirectoryProperty

    @get:Inject
    abstract val exec: ExecOperations

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun run() {
        exec.exec {
            workingDir = workspace.get().asFile
            environment(this@CargoTask.environment.get())
            commandLine(listOf("cargo") + arguments.get())
        }
    }
}

// The core for the APK: 16 KB-aligned, release profile, without the bindgen CLI's dependencies.
// Its own target directory keeps this NDK build apart from the workspace's host builds.
val cargoNdkBuild = tasks.register<CargoTask>("cargoNdkBuild") {
    workspace.set(workspaceDir)
    val pageSize = "-C link-arg=-Wl,-z,max-page-size=16384"
    environment.set(
        androidComponents.sdkComponents.ndkDirectory.map {
            mapOf(
                "ANDROID_NDK_HOME" to it.asFile.path,
                "CARGO_TARGET_DIR" to cargoTargetDir.resolve("android").path,
                "CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS" to pageSize,
                "CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS" to pageSize,
            )
        },
    )
    arguments.set(
        outputDir.locationOnly.map {
            listOf(
                "ndk", "-t", "arm64-v8a", "-t", "x86_64", "-P", "26", "-o", it.asFile.path,
                "build", "-p", "notae-app-core", "--lib", "--release", "--no-default-features",
            )
        },
    )
}

// The core for the JVM unit tests, which JNA finds through `jna.library.path`. Default features,
// so it is the same build `cargo test` and the bindgen run below use.
val hostLibDir = cargoTargetDir.resolve("debug")
val cargoHostBuild = tasks.register<CargoTask>("cargoHostBuild") {
    workspace.set(workspaceDir)
    arguments.set(listOf("build", "-p", "notae-app-core", "--lib"))
}

val generateUniffiBindings = tasks.register<CargoTask>("generateUniffiBindings") {
    dependsOn(cargoHostBuild)
    workspace.set(workspaceDir)
    arguments.set(
        outputDir.locationOnly.map {
            listOf(
                "run", "-q", "-p", "notae-app-core", "--bin", "uniffi-bindgen", "--",
                "generate", "--library", hostLibDir.resolve("libnotae_app_core.so").path,
                "--language", "kotlin", "--no-format", "--out-dir", it.asFile.path,
            )
        },
    )
}

androidComponents {
    onVariants { variant ->
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargoNdkBuild, CargoTask::outputDir)
        variant.sources.kotlin?.addGeneratedSourceDirectory(generateUniffiBindings, CargoTask::outputDir)
    }
}

tasks.withType<Test>().configureEach {
    dependsOn(cargoHostBuild)
    systemProperty("jna.library.path", hostLibDir.path)
    // Robolectric's SDK 36 sandbox reaches into JDK internals, which JDK 25 does not export.
    jvmArgs("--add-exports=java.base/jdk.internal.access=ALL-UNNAMED")
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.appcompat)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui)
    implementation(libs.navigation3.ui)
    // The Android build of JNA, with its native dispatch library for each ABI.
    implementation(libs.jna) { artifact { type = "aar" } }

    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.ui.test.junit4)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
    testImplementation(libs.roborazzi.junit.rule)
    debugImplementation(libs.compose.ui.test.manifest)
    // The desktop JNA jar, whose dispatch library runs the host core in unit tests.
    testImplementation(libs.jna)
}
