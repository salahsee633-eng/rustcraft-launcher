# الذاكرة والأداء على الأجهزة الضعيفة

## 1. الميزانية (هدف جهاز 3 GB RAM، أندرويد 11+)

| المكوّن | الحجم التقريبي | ملاحظة |
|---|---|---|
| Slint UI + Rust core | 15–25 MB | software renderer، بلا WebView |
| OpenJDK (jlink) resident | 20–35 MB | جداول + metaspace؛ الـ heap منفصل |
| Metaspace | 40–80 MB | يعتمد على عدد المودات |
| JVM heap `-Xmx512M` | حتى 512 MB | يبدأ أقل وينمو |
| LWJGL + gl4es + اللعبة | 80–120 MB | مع رندر 720p و6 chunks |
| **المجموع** | **≈ 350–450 MB** | 1.8.9 / 1.12.2 |
| 1.20.1 + Fabric خفيف | 500–650 MB | يحتاج 4 GB فعلياً |

> أرقام واقعية: JVM وحده يأخذ 120–200 MB قبل اللعبة. أي وعد بـ "50 MB" غير صحيح.

## 2. قرارات تقلّل الذاكرة من الأساس

1. **واجهة Rust أصلية (Slint) بدل Java/WebView** — الواجهة التقليدية تكلّف
   10–20 MB بدل 100+ MB لـ Chromium.
2. **بلا `tokio` ولا async runtime** — التنزيل بـ 4 خيوط OS فقط، ثم تختفي.
3. **تنزيل ستريمنغ** — لا `Vec<u8>` لملف 300 MB؛ الكتابة على القرص أثناء التنزيل.
4. **مشاركة client jar** — إصدارات Fabric/Forge لا تنسخ jar الأب (توفير ~20 MB
   لكل instance).
5. **تخطّي الملفات السليمة** بالهاش بدل إعادة تنزيلها أو قراءتها كاملة.
6. **hard-link للأصول القديمة** (`assets/virtual/legacy`) بدل النسخ.
7. **`inheritsFrom` يُدمج عند الطلب**، وما نحتفظ بنسخ JSON مضاعفة.

## 3. إعدادات الجهاز

### 3.1 الـ heap حسب الرام

| رام الجهاز | `-Xmx` مقترح | إصدار |
|---|---|---|
| 2 GB | 256–384 MB | 1.8.9 فقط، رندر 2 chunks |
| 3 GB | 384–512 MB | 1.8.9 / 1.12.2 |
| 4 GB | 768 MB | 1.16.5 / 1.20 مع Fabric خفيف |
| 6 GB+ | 1024–1536 MB | 1.20+ ومودباك خفيف |

`-Xms` = نصف `-Xmx` تقريباً، حتى لا يتضخم الـ heap بسرعة.

### 3.2 أعلام JVM

```text
-XX:+UseSerialGC
-XX:MaxMetaspaceSize=128m
-XX:ReservedCodeCacheSize=64m
-XX:+ExitOnOutOfMemoryError
-Dfml.ignoreInvalidMinecraftCertificates=true   (Forge فقط)
```

`UseSerialGC` هو الافتراضي في الكود. G1 يستهلك ذاكرة إضافية و pause أقل فائدة
على 2–4 أنوية.

### 3.3 تقليص OpenJDK بـ jlink

```bash
jlink \
  --add-modules java.base,java.desktop,java.logging,java.management,\
java.naming,java.net.http,java.prefs,java.rmi,java.scripting,\
java.security.jgss,java.security.sasl,java.sql,java.transaction.xa,\
java.xml,jdk.crypto.ec,jdk.unsupported,jdk.zipfs,java.instrument \
  --strip-debug --no-man-pages --no-header-files --compress=2 \
  --output jre-android
```

- `java.desktop` ضروري (AWT/Swing تُستخدم في بعض مسارات اللعبة واللوجو).
- `java.instrument` ضروري لبعض اللوادر.
- `jdk.unsupported` ضروري لـ `sun.misc.Unsafe` (Netty/Mixin).
- لا تضف `jdk.jfr` ولا `jdk.javadoc` ولا `jdk.compiler`.

### 3.4 الرسوميات

| الجهاز | Renderer | قرار |
|---|---|---|
| Adreno قديم/متوسط | gl4es | أخف، GL 2.1 كافي لـ 1.8.9 |
| Adreno حديث | ANGLE | توافق GLES 3 ممتاز |
| Snapdragon 8+ / Vulkan 1.1 | Zink | أقرب لسلوك PC لكن أثقل |
| Mali | gl4es أو ANGLE | جرّب الاثنين؛ Zink غالباً مشاكل |

خيارات إضافية:

- دقة الرسم 720p بدل 1080p (أو scale 0.75) — أكبر توفير في VRAM/RAM.
- Render distance 4–6 chunks، بلا shaders، بلا mipmaps على 2 GB.
- `-Dorg.lwjgl.opengl.Display.allowSoftwareOpenGL=false` غير مطلوب مع gl4es.

## 4. قياس فعلي (لازم قبل أي تحسين)

```bash
adb shell dumpsys meminfo org.rustcraft.launcher
adb shell cat /proc/<pid>/status | grep -E 'VmRSS|VmHWM'
adb logcat -s RustCraft:V
```

قِس ثلاث نقاط: بعد فتح الواجهة، بعد الدخول للعالم، وبعد 5 دقائق لعب. الهدف:

- واجهة فقط: < 40 MB RSS.
- داخل القائمة الرئيسية للعبة: < 300 MB.
- داخل العالم (1.8.9، 512M): < 450 MB.

## 5. أخطاء شائعة ترفع الذاكرة

- `-Xmx2G` على جهاز 3 GB → نظام يقتل التطبيق (OOM killer).
- تشغيل shaders على gl4es → بطء شديد وليس ذاكرة فقط.
- تحميل نسخة مع مودباك 200+ مود على 3 GB.
- إبقاء نسخ JRE متعددة (8/17/21) مضمّنة؛ نزّل فقط المطلوب لكل إصدار.
- نسيان `assets` القديمة: كل إصدار يضيف مئات MB على القرص (ليس RAM لكن يهم
  الأجهزة 16/32 GB).
