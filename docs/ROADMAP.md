# خريطة الطريق

كل مرحلة لها "تعريف إنجاز" قابل للاختبار. لا ننتقل لمرحلة قبل تحقيقه.

## M0 — الهيكل (منجز في هذا المستودع)

- [x] Workspace Rust: core / cli / ui.
- [x] قراءة manifest، rules، classpath، دمج `inheritsFrom`.
- [x] تنزيل متوازي + SHA-1 + استئناف.
- [x] تسجيل دخول Microsoft كامل.
- [x] بناء أمر Java.
- [x] واجهة Slint بالشكل القديم.
- [x] Gradle module + AndroidManifest.

**إنجاز:** `cargo test -p rustcraft-core` ينجح، و`rustcraft plan 1.20.1` يطبع أمراً
صحيحاً على الكمبيوتر.

## M1 — تشغيل حقيقي على الكمبيوتر

- [ ] `rustcraft install 1.20.1 && rustcraft launch 1.20.1` يفتح اللعبة.
- [ ] إدارة JRE لكل إصدار (8 / 17 / 21) بدل الاعتماد على `java` في PATH.
- [ ] تحميل natives لـ Linux/Windows بشكل موثوق (اختبار على نظامين).
- [ ] التقاط اللوج وعرضه في الواجهة.

**إنجاز:** يكمل تثبيت 1.20.1 و1.8.9 ثم يفتح القائمة الرئيسية داخل العالم.

## M2 — تكافؤ الواجهة

- [ ] شاشة تثبيت نسخة مع شريط تقدّم حقيقي (`DownloadStats`).
- [ ] شاشة حساب Microsoft (كود + انتظار + نجاح).
- [ ] إعدادات: RAM، الدقة، Java args، اختيار النسخة.
- [ ] سجل مباشر داخل التطبيق.
- [ ] Instance folders (كل نسخة/mods معزولة).

**إنجاز:** مستخدم جديد يثبّت ويلعب من الواجهة فقط، بدون CLI.

## M3 — جسر JVM على أندرويد (أصعب مرحلة)

- [ ] بناء OpenJDK arm64 مصغّر بـ jlink ووضعه في `jniLibs`/assets.
- [ ] `JNI_CreateJavaVM` من Rust داخل عملية NativeActivity.
- [ ] تمرير `ANativeWindow` إلى GLFW stub (LWJGL fork).
- [ ] استبدال LWJGL المكتبي بنسخة أندرويد في الـ classpath
      (`LaunchOptions.classpath_exclude = ["org.lwjgl"]`).
- [ ] gl4es كأول renderer + `-Dorg.lwjgl.opengl.libname=libgl4es.so`.
- [ ] إدخال لمس أساسي (مؤشر + نقر).
- [ ] إغلاق/استئناف نظيف للـ JVM.

**إنجاز:** 1.8.9 تفتح القائمة الرئيسية على جهاز arm64 حقيقي، ثم عالم جديد.

> المخاطرة الأكبر: السطح ودورة الحياة. خطة بديلة: `SurfaceView` صغير يُنشأ
> عبر JNI ويُمرَّر إلى GLFW بدل نافذة NativeActivity.

## M4 — الحساب والأمان

- [ ] تخزين التوكنات في Android Keystore (AES-GCM).
- [ ] Refresh تلقائي قبل الانتهاء.
- [ ] شاشة "امتلك اللعبة؟" عند 404 من profile.
- [ ] دعم حسابات متعددة.

## M5 — المودات والمحتوى

- [ ] Fabric وQuilt installer (بدون تشغيل Java installer إن أمكن).
- [ ] Forge/NeoForge (يحتاج processors، أصعب).
- [ ] Modrinth/CurseForge search + install.
- [ ] Resource packs وshaders (Iris) مع تحذير أداء.
- [ ] Import/export instance.

## M6 — الأداء والتوافق

- [ ] ملف تعريف جهاز تلقائي (RAM، SoC، renderer).
- [ ] اختيار Zink/ANGLE/gl4es مع إعادة تشغيل آمنة.
- [ ] Overlay تحكم قابل للتخصيص + يد بلوتوث.
- [ ] قياسات ذاكرة موثّقة على 5 أجهزة (2/3/4/6/8 GB).
- [ ] وضع "Low-end": 1.8.9، 2 chunks، 384M، gl4es.
- [ ] توقيع APK، تحديثات، سياسة خصوصية.

## أفكار مؤجلة

- لانشر سطح مكتب بنفس الواجهة (المكوّن موجود).
- دعم x86_64 (Chromebooks/محاكيات).
- Streaming من PC (غير مرتبط).

## معايير قبول عامة

1. لا ملف من Mojang داخل المستودع أو الـ APK.
2. أي تنزيل يُتحقق منه بالهاش.
3. لا عملية تعمل في الخلفية بعد الخروج.
4. التشغيل البارد للواجهة < 1.5 ثانية على جهاز 3 GB.
5. استهلاك الواجهة وحدها < 40 MB RSS.
