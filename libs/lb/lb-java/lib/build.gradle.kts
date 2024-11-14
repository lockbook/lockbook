import org.gradle.internal.os.OperatingSystem

plugins {
    // Apply the java-library plugin for API and implementation separation.
    `java-library`
}

repositories {
    mavenCentral()
}

dependencies {
    // JUnit test framework
    testImplementation("junit:junit:4.13.2")

    // Expose the commons math library to consumers
    api("org.apache.commons:commons-math3:3.6.1")

    // Internal dependencies
    implementation("com.google.guava:guava:33.2.1-jre")
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(17)
    }
}

tasks.register("buildNativeLibs") {
    group = "build"

    doLast {
        val rustProjectDir = file("../")

        exec {
            workingDir = rustProjectDir

            commandLine("cargo", "ndk",
                "-t", "armeabi-v7a",
                "-t", "arm64-v8a",
                "-t", "x86",
                "-t", "x86_64",
                "-o", "./lib/src/main/jniLibs",
                "build", "--release"
            )
        }
    }
}

tasks.register("buildTestNativeLibs") {
    doLast {
        val rustProjectDir = file("../")
        val os = OperatingSystem.current()

        val libName = when {
            os.isMacOsX -> "liblb_java.dylib"
            os.isLinux -> "liblb_java.so"
            os.isWindows -> "liblb_java.dll"
            else -> throw Exception("Unsupported testing platform.")
        }

        val targetDir = file("./src/main/jniLibs/desktop")
        targetDir.mkdirs()

        exec {
            workingDir = rustProjectDir
            commandLine("cargo", "build", "--lib", "--release")
        }

        val sourceFile = file("${rustProjectDir}/../../../target/release/$libName")
        if (sourceFile.exists()) {
            copy {
                from(sourceFile)
                into(targetDir)
                rename { libName }
            }
        } else {
            throw Exception("Build failed or library file not found: $sourceFile")
        }
    }
}

tasks.withType<Test>().all {
    jvmArgs("-Djava.library.path=./src/main/jniLibs/desktop")
}
