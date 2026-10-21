# JNA 通过反射找 Native、Structure、Callback 等，名字不能被改
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }
-dontwarn java.awt.**
# UniFFI 生成的绑定：JNA 按方法名找 Rust 导出的函数，回调接口也靠反射
-keep class uniffi.** { *; }
# kotlinx.serialization 的序列化器
-keepattributes *Annotation*, InnerClasses
-keepclassmembers class com.fleetinggleam.android.** {
    *** Companion;
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class com.fleetinggleam.android.**$$serializer { *; }
