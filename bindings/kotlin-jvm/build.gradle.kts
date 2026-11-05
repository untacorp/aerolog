// Copyright 2026 AeroLog Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

plugins {
    kotlin("jvm")
    `maven-publish`
    signing
}

group = "io.github.untacorp"
version = "0.1.0"

base {
    archivesName.set("aerolog-jvm")
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
    withSourcesJar()
    withJavadocJar()
}

sourceSets {
    main {
        kotlin {
            srcDirs("../kotlin/src/main/kotlin")
        }
    }
}

tasks.withType<ProcessResources> {
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}

dependencies {
    implementation("net.java.dev.jna:jna:5.14.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0")

    testImplementation("junit:junit:4.13.2")
}

publishing {
    publications {
        create<MavenPublication>("mavenJava") {
            groupId = "io.github.untacorp"
            artifactId = "aerolog-jvm"
            version = "0.1.0"

            from(components["java"])

            pom {
                name.set("aerolog-jvm")
                description.set("AeroLog: Universal crash-resilient local-first append-only storage engine for JVM")
                url.set("https://github.com/untacorp/aerolog")

                licenses {
                    license {
                        name.set("The Apache License, Version 2.0")
                        url.set("http://www.apache.org/licenses/LICENSE-2.0.txt")
                    }
                }

                developers {
                    developer {
                        id.set("aerolog-authors")
                        name.set("AeroLog Authors")
                        email.set("maintainers@aerolog.dev")
                    }
                }

                scm {
                    connection.set("scm:git:git://github.com/untacorp/aerolog.git")
                    developerConnection.set("scm:git:ssh://github.com/untacorp/aerolog.git")
                    url.set("https://github.com/untacorp/aerolog")
                }
            }
        }
    }

    repositories {
        maven {
            name = "staging"
            url = uri(rootProject.layout.buildDirectory.dir("staging-repo"))
        }
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/untacorp/aerolog")
            credentials {
                username = System.getenv("GITHUB_ACTOR") ?: ""
                password = System.getenv("GITHUB_TOKEN") ?: ""
            }
        }
    }
}

val signingKey = System.getenv("GPG_PRIVATE_KEY")
val signingPassword = System.getenv("GPG_PASSPHRASE") ?: ""
if (!signingKey.isNullOrBlank()) {
    signing.useInMemoryPgpKeys(signingKey, signingPassword)
    signing.sign(publishing.publications["mavenJava"])
} else {
    signing.isRequired = false
}
