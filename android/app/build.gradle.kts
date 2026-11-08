// App Android do CamFácil IA: WebView do painel + recursos nativos
// (pareamento por QR/endereço, alertas em notificação, câmeras ao vivo com
// troca automática para foto em conexão fraca) e, opcionalmente, o próprio
// servidor (núcleo Rust + ffmpeg em jniLibs, gerados por
// scripts/android-nucleo.sh). Sem AndroidX para o APK ficar pequeno.
import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// A versão do app segue a do Cargo.toml (fonte única de versão do projeto).
val versaoCargo: String = File(rootDir, "../Cargo.toml").readLines()
    .first { it.startsWith("version") }
    .substringAfter('"').substringBefore('"')
val partes = versaoCargo.split(".").map { it.takeWhile(Char::isDigit).toIntOrNull() ?: 0 }
val codigoVersao = partes.getOrElse(0) { 0 } * 10000 + partes.getOrElse(1) { 0 } * 100 + partes.getOrElse(2) { 0 }

// Assinatura de release: keystore opcional por variáveis de ambiente
// (secrets do GitHub). Sem elas, assina com a chave de debug.
val keystore = System.getenv("CFIA_KEYSTORE_FILE")?.let(::File)?.takeIf { it.isFile }

android {
    namespace = "br.com.ysafe.camfacil"
    compileSdk = 35
    buildToolsVersion = "35.0.0"

    defaultConfig {
        applicationId = "br.com.ysafe.camfacil"
        minSdk = 24
        targetSdk = 35
        versionCode = maxOf(codigoVersao, 1)
        versionName = versaoCargo
        resourceConfigurations += listOf("pt-rBR", "pt")
    }

    signingConfigs {
        if (keystore != null) {
            create("cfia") {
                storeFile = keystore
                storePassword = System.getenv("CFIA_KEYSTORE_SENHA")
                keyAlias = System.getenv("CFIA_KEY_ALIAS")
                keyPassword = System.getenv("CFIA_KEY_SENHA") ?: System.getenv("CFIA_KEYSTORE_SENHA")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("cfia") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    packaging {
        resources.excludes += listOf("META-INF/**", "kotlin/**", "DebugProbesKt.bin")
        // Servidor no aparelho (ADR 0010): libcamfacil.so e libffmpeg.so são
        // executáveis. Precisam ser extraídos para o nativeLibraryDir (único
        // lugar de onde o Android deixa executar) e já vêm sem símbolos.
        jniLibs {
            useLegacyPackaging = true
            keepDebugSymbols += "**/*.so"
        }
    }
    dependenciesInfo {
        includeInApk = false
        includeInBundle = false
    }
}

dependencies {
    // Leitor de QR (só o núcleo, ~550 KB antes do R8; sem câmera embutida).
    implementation("com.google.zxing:core:3.5.3")
}
