# Android build

## 1. المتطلبات

- Android Studio أحدث إصدار (AGP 8.7+, Gradle 8.9+, JDK 17).
- Android NDK **26.1.10909125** (أو عدّل `ndkVersion` في
  `app/build.gradle.kts`).
- Rust + targets:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-ndk
```

## 2. بناء مكتبة Rust

من جذر المستودع:

```bash
cargo ndk -t arm64-v8a -t armeabi-v7a \
  -o android/app/src/main/jniLibs \
  build -p rustcraft-ui --release --no-default-features --features android
```

هذا ينتج:

```
android/app/src/main/jniLibs/arm64-v8a/librustcraft_ui.so
android/app/src/main/jniLibs/armeabi-v7a/librustcraft_ui.so
```

## 3. بناء APK

افتح مجلد `android/` في Android Studio، أو:

```bash
cd android
gradle wrapper        # أول مرة فقط (أو استخدم Gradle المضمّن في Android Studio)
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## 4. ماذا يعمل الآن؟

- التطبيق يفتح واجهة Slint كاملة (شكل اللانشر القديم).
- لا يوجد بعد: JVM، تنزيل نسخ من داخل الواجهة، تسجيل دخول، تشغيل اللعبة.
  هذي المرحلة **M3** في [`../docs/ROADMAP.md`](../docs/ROADMAP.md).

الضغط على Play حالياً يعرض رسالة أن جسر JVM غير منفّذ. هذا مقصود حتى لا يوهم
المستخدم بوجود لعبة.

## 5. ملاحظات تقنية

- **NativeActivity:** لا يوجد كود Java في التطبيق (`android:hasCode="false"`).
  نقطة الدخول هي `android_main` من `android-activity`، واسم المكتبة في
  `meta-data android.app.lib_name` يجب أن يطابق `[lib] name = "rustcraft_ui"`.
- **استخراج المكتبات:** `useLegacyPackaging = true` و`extractNativeLibs="true"`
  مطلوبان لأن JRE/gl4es تُحمَّل من مسار على القرص، وليس من داخل APK.
- **Slint Android backend:** الخاصية `backend-android-activity-06` وإصدار
  `android-activity` مرتبطان. إذا حدّثت Slint، راجع دليلها وقد تحتاج تغيير
  `slint::android::init` أو رقم الـ backend.
- **التوجيه:** الواجهة portrait؛ عند تشغيل اللعبة (M3) نمرّر
  `sensorLandscape` عبر `ActivityInfo` أو نغيّر
  `android:screenOrientation` برمجياً.
- **التوقيع:** استخدم keystore خاص؛ لا تشارك debug keystore.

## 6. الخطوة التالية (M3)

1. ابنِ OpenJDK arm64 بـ jlink وضعه في assets (يُستخرج أول تشغيل).
2. أضف `jni = "0.21"` إلى `crates/ui` (أو crate جديد `rustcraft-android`).
3. أنشئ JVM من `LaunchPlan`، مرّر `ANativeWindow` إلى LWJGL fork.
4. أضف `libgl4es.so` و`libopenal.so` إلى `jniLibs`.

التفاصيل في [`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) القسم 3.3.
