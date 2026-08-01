plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "c.cyrio.android"
    compileSdk = 34

    defaultConfig {
        applicationId = "c.cyrio.android"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "1.0.0"

        // 只构建 arm64-v8a（匹配 Rust 交叉编译目标）
        ndk {
            abiFilters += "arm64-v8a"
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
        debug {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    // JNI 原生库放在 src/main/jniLibs/arm64-v8a/libcyrio_jni.so
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    // 打包时不压缩 .so（Android 16 页对齐要求）
    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}

dependencies {
    // AndroidX 基础库（Fragment + AppCompatActivity）
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-ktx:1.9.3")
    implementation("androidx.fragment:fragment-ktx:1.8.5")
    implementation("androidx.recyclerview:recyclerview:1.3.2")

    // Material Components（BottomNavigationView）
    implementation("com.google.android.material:material:1.12.0")

    // JSON 解析（Android 自带 org.json，无需额外依赖）
    // 文件选择器使用 Android 系统自带的 Storage Access Framework
}
