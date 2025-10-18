plugins {
    kotlin("jvm") version "1.9.10"
    application
}

group = "com.bitchat"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    // Command line argument parsing
    implementation("com.github.ajalt.clikt:clikt:4.2.1")
    
    // Coroutines for async operations
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")
    
    // JSON handling
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.0")
    
    // WebSocket client for Nostr
    implementation("io.ktor:ktor-client-core:2.3.4")
    implementation("io.ktor:ktor-client-cio:2.3.4")
    implementation("io.ktor:ktor-client-websockets:2.3.4")
    implementation("io.ktor:ktor-client-logging:2.3.4")
    
    // Logging
    implementation("ch.qos.logback:logback-classic:1.4.11")
    implementation("io.github.microutils:kotlin-logging-jvm:3.0.5")
    
    // Crypto (Bouncy Castle)
    implementation("org.bouncycastle:bcprov-jdk15on:1.70")
    implementation("org.bouncycastle:bcpkix-jdk15on:1.70")
    
    testImplementation(kotlin("test"))
}

tasks.test {
    useJUnitPlatform()
}

// Configure Java and Kotlin compilation
java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
    kotlinOptions {
        jvmTarget = "17"
    }
}

application {
    mainClass.set("com.bitchat.cli.MainKt")
    applicationName = "bitchat-kotlin-cli"
}

// Create a fat JAR for easy distribution
tasks.jar {
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    
    manifest {
        attributes["Main-Class"] = "com.bitchat.cli.MainKt"
    }
    
    from(configurations.runtimeClasspath.get().map { if (it.isDirectory) it else zipTree(it) }) {
        exclude("META-INF/*.SF")
        exclude("META-INF/*.DSA")
        exclude("META-INF/*.RSA")
    }
}