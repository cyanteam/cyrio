# cyrio ProGuard 规则

# 保留 JNI native 方法（Kotlin external 函数）
-keepclasseswithmembernames class * {
    native <methods>;
}

# 保留 CyrioNative（JNI 桥接类，类名不能被混淆）
-keep class xyz.cyanteam.cyrio.jni.CyrioNative { *; }

# 保留 CyrioUsbHelper（Rust 侧通过 JNI 反射调用其静态方法）
-keep class xyz.cyanteam.cyrio.usb.CyrioUsbHelper { *; }

# 保留数据模型（JSON 反序列化需要字段名）
-keep class xyz.cyanteam.cyrio.model.** { *; }

# Kotlin 元数据
-keepattributes *Annotation*, InnerClasses, Signature, Exceptions
-dontwarn kotlin.**
