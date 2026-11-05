# Multiplatform JVM Guide (Java & Kotlin)

AeroLog provides universal, native embedded storage for Java and Kotlin on the JVM across Linux, macOS, and Windows.

Native dynamic libraries are bundled directly inside the JAR archive and unpacked automatically at runtime by JNA, requiring no manual system library installation or C/C++ compilation.

---

## 1. Supported JVM Platforms

| Operating System | Architecture | Native Dynamic Library | Status |
| :--- | :--- | :--- | :--- |
| **Linux Desktop & Server** | `x86_64` (Intel / AMD) | `linux-x86-64/libaerolog_uniffi.so` | Bundled |
| **Linux Edge & IoT** | `aarch64` / `arm64` (Raspberry Pi) | `linux-aarch64/libaerolog_uniffi.so` | Bundled |
| **macOS (Apple Silicon)** | `arm64` (M1 / M2 / M3 / M4) | `darwin-aarch64/libaerolog_uniffi.dylib` | Bundled |
| **macOS (Intel)** | `x86_64` | `darwin-x86-64/libaerolog_uniffi.dylib` | Bundled |
| **Windows Desktop & Server**| `x86_64` (64-bit) | `win32-x86-64/aerolog_uniffi.dll` | Bundled |

---

## 2. Installation

### Gradle (Kotlin DSL)

```kotlin
dependencies {
    implementation("io.aerolog:aerolog-jvm:0.1.0")
    implementation("net.java.dev.jna:jna:5.14.0")
}
```

### Maven (`pom.xml`)

```xml
<dependency>
    <groupId>io.aerolog</groupId>
    <artifactId>aerolog-jvm</artifactId>
    <version>0.1.0</version>
</dependency>
<dependency>
    <groupId>net.java.dev.jna</groupId>
    <artifactId>jna</artifactId>
    <version>5.14.0</version>
</dependency>
```

---

## 3. Usage Example (Kotlin)

```kotlin
import io.aerolog.AeroLog
import java.io.File

fun main() {
    val dataDir = File("build/aerolog_db")
    
    AeroLog.open(dataDir).use { db ->
        // Key-Value operations
        db.put("settings", "theme", "dark")
        val theme = db.getString("settings", "theme")
        println("Loaded theme: $theme")

        // Append-only event streams
        val stream = db.stream("audit_events")
        val entry = stream.append("USER_LOGIN: uid=42")
        println("Appended event sequence: ${entry.sequenceNumber}")

        val recent = stream.readRecent(5u)
        recent.forEach { record ->
            println("Event payload: ${record.payload.decodeToString()}")
        }
    }
}
```

---

## 4. Usage Example (Java)

```java
import io.aerolog.AeroLog;
import io.aerolog.AeroLogStream;
import io.aerolog.uniffi.FfiStreamEntry;
import java.io.File;
import java.util.List;

public class Main {
    public static void main(String[] args) {
        File dataDir = new File("build/aerolog_db");

        try (AeroLog db = AeroLog.Companion.open(dataDir, 0L)) {
            // Put and Get
            db.put("configs", "timeout_ms", "5000");
            String timeout = db.getString("configs", "timeout_ms");
            System.out.println("Config timeout: " + timeout);

            // Append-only stream
            AeroLogStream stream = db.stream("telemetry");
            FfiStreamEntry entry = stream.append("METRIC:cpu=12%".getBytes());
            System.out.println("Recorded entry sequence: " + entry.getSequenceNumber());
        }
    }
}
```
