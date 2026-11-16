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
- **M3.1 مكتمل:** زر Play يشغّل اختبار جسر JVM حقيقي. التطبيق يستخرج JRE8
  (arm64) من داخل الـAPK أول مرة، ثم يبدأ الـJVM عبر `JLI_Launch` من
  `libjli.so`، ويشغّل `rustcraft.BridgeTest` الذي يكتب إصدار جافا في ملف.
  تظهر النتيجة في شريط الحالة، مثال: `JVM OK — java.version=1.8.0_...`.
- لا يوجد بعد: تشغيل ماينكرافت نفسه (LWJGL/gl4es)، تنزيل النسخ من داخل
  الواجهة، وتسجيل الدخول. هذه الخطوة التالية داخل **M3**.

JRE8 مأخوذ من حزمة PojavLauncher المفتوحة المصدر ويُفكّ ضغطه في CI، ثم
تُفكّ ملفات `.pack` بأداة `unpack200` من JDK 8. لا تُضمّن أصول Mojang.

## 5. ملاحظات تقنية

- **NativeActivity:** لا يوجد كود Java في التطبيق (`android:hasCode="false"`).
  نقطة الدخول هي `android_main` من `android-activity`، واسم المكتبة في
  `meta-data android.app.lib_name` يجب أن يطابق `[lib] name = "rustcraft_ui"`.
- **استخراج المكتبات:** `useLegacyPackaging = true` و`extractNativeLibs="true"`
  مطلوبان لأن JRE/gl4es تُحمَّل من مسار على القرص، وليس من داخل APK.
- **JRE:** ملف `assets/jre8.zip` يُبنى في CI (انظر الـworkflow). المسار
  `files/jre8`، والمكتبات تُحمّل بـ`dlopen` مطلق مع `RTLD_GLOBAL`، لأن
  أندرويد لا يحدّث مسار البحث بعد بدء العملية.
- **Slint Android backend:** الخاصية `backend-android-activity-06` وإصدار
  `android-activity` مرتبطان. إذا حدّثت Slint، راجع دليلها وقد تحتاج تغيير
  `slint::android::init` أو رقم الـ backend.
- **التوجيه:** الواجهة portrait؛ عند تشغيل اللعبة (M3) نمرّر
  `sensorLandscape` عبر `ActivityInfo` أو نغيّر
  `android:screenOrientation` برمجياً.
- **التوقيع:** يوجد مفتاح preview ثابت في `app/rustcraft-debug.keystore`
  (كلمة السر `android`) حتى تُثبَّت التحديثات فوق بعضها دون مسح البيانات.
  هذا **ليس** مفتاح نشر؛ بناء release يحتاج keystore خاص وسرّ في CI.

## 6. الخطوة التالية (M3.2)

1. أضف LWJGL3 Android fork (`liblwjgl.so`، `liblwjgl_opengl.so`) و`libgl4es.so`
   إلى `jniLibs`.
2. ابنِ `LaunchPlan` الحقيقي (1.8.9، gl4es، 1024MB) ومرّر `ANativeWindow`
   إلى LWJGL/android-activity.
3. شغّل `net.minecraft.client.main.Main` عبر `JLI_Launch` مع classpath اللعبة.
4. اختبر على M33، ثم أضف شاشة تقدم التنزيل وتسجيل الدخول.

التفاصيل في [`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) القسم 3.3.
