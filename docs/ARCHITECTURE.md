# المعمارية — RustCraft Launcher

## 1. الصورة الكاملة

```
┌──────────────────────────────────────────────────────────────┐
│ Slint UI (Rust)        crates/ui                              │
│  الشاشة القديمة: نسخ، حساب، Play، سجل، إعدادات                │
└───────────────▲──────────────────────────────┬───────────────┘
                │ properties / callbacks       │ Play
┌───────────────┴──────────────────────────────▼───────────────┐
│ rustcraft-core (Rust, بدون UI)                                │
│  manifest · rules · install · download · auth · launch        │
└───────────────┬──────────────────────────────────────────────┘
                │ LaunchPlan { classpath, mainClass, args, env }
┌───────────────▼──────────────────────────────────────────────┐
│ Android platform layer (Rust + JNI)  ← المرحلة M3             │
│  ANativeWindow · JNI_CreateJavaVM · input · audio · storage   │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│ OpenJDK (مصغّر بـ jlink)                                      │
│  LWJGL android fork · GLFW stub · gl4es/Zink · OpenAL         │
│  Minecraft client jar                                          │
└──────────────────────────────────────────────────────────────┘
```

## 2. ليش Rust + JVM، وليش مو Rust خالص؟

Minecraft: Java Edition يُوزّع كبايت كود JVM (`.class` داخل jar). أندرويد يشغّل
ART، وهو **لا** ينفّذ بايت كود Java 17. أي لانشر — Pojav أو غيره — لازم يضمّ
توزيعة OpenJDK مبنية لأندرويد (arm64) وينشئ JVM داخل العملية. حتى لو أعدنا كتابة
كل شي بـ Rust، تبقى هذي الطبقة إجبارية.

كذلك اللعبة تنادي OpenGL 2.1/3.3، وأندرويد يوفّر OpenGL ES + Vulkan. لذلك لازم
مترجم: gl4es (GL→GLES)، Zink (GL→Vulkan)، أو ANGLE (GLES→Vulkan). هذي مكتبات
C موجودة ومجرّبة، وإعادة كتابتها بـ Rust مشروع أكبر من اللانشر نفسه.

**قرار التصميم:** Rust يملك كل شي *حول* اللعبة، ويعيد استخدام الطبقة السفلية
المجرّبة. هذا يعطينا:

- واجهة و منطق لانشر أخف بمراحل من واجهة Java/WebView.
- كود واحد للكمبيوتر والأندرويد (نفس `rustcraft-core`).
- تحكم كامل بالذاكرة: تنزيلات ستريمنغ، بلا runtime غير ضروري، بلا GC للواجهة.

## 3. مسؤوليات الطبقات

### 3.1 rustcraft-core

| ملف | المسؤولية |
|---|---|
| `manifest.rs` | تحليل `version.json`، دمج `inheritsFrom` (Fabric/Quilt/Forge) بقواعد: المكتبات وقوائم arguments تُجمع، والقيم المفردة تأخذ Child. |
| `rules.rs` | تقييم `rules` حسب `os` و`features`. أندرويد يُقدَّم كـ `linux` + `arm64`. |
| `install.rs` | تنزيل JSON النسخة والأب، client jar، المكتبات، asset index، كل الـ objects، ثم فك natives على الكمبيوتر. |
| `download.rs` | عمّال تنزيل (4 افتراضياً)، كتابة مباشرة للقرص، `SHA-1` أثناء الكتابة، ملف `.part`، تخطّي الموجود السليم. |
| `auth.rs` | device code → Microsoft token → Xbox Live → XSTS → Minecraft access token → profile. |
| `launch.rs` | بناء أمر Java كامل مع تعويض `${...}`، classpath، `-Xmx/-Xms`, `-cp`. |
| `session.rs` | حفظ الحساب (صلاحيات 0600 على Unix). |

### 3.2 crates/ui

Slint، renderer برمجي (software). لا WebView ولا JS. الواجهة تعرض:

- قائمة النسخ (المثبّتة تُقرأ من `versions/`).
- الحساب، الأخبار، زر Play.
- أزرار سفلية: Home / Instances / Refresh / Settings.

كل زر يمرّر callback إلى Rust. ملف التصميم `ui/app.slint` ويوصف بصرياً في
[`UI-LEGACY.md`](UI-LEGACY.md).

### 3.3 Android platform layer (M3)

هذا الجزء **غير منفّذ** في هذا المستودع، وهذا تصميمه المطلوب:

1. **البدء:** `android_main(AndroidApp)` من `android-activity` ← إنشاء واجهة
   Slint على `ANativeWindow`.
2. **الضغط على Play:** إخفاء رسم Slint، ثم إنشاء JVM داخل نفس العملية عبر JNI
   (`JNI_CreateJavaVM`) مع:
   - `-Djava.class.path` = classpath من `LaunchPlan` بعد استبدال LWJGL المكتبي
     بنسخة أندرويد.
   - `-Djava.library.path` = مجلد JRE libs + natives.
   - `-Xmx/-Xms` حسب إعدادات الجهاز.
3. **السطح:** تمرير `ANativeWindow` (و`Surface` JNI) إلى GLFW stub الخاص بـ
   LWJGL fork. اللعبة ترسم مباشرة على نافذة NativeActivity.
4. **الرسوميات:** اختيار واحد من:
   - `libgl4es.so` — أخف وأسرع على أجهزة قديمة، GL 2.1.
   - `libEGL`/ANGLE — توافق أعلى مع GLES 3.
   - Zink — يحتاج Vulkan 1.1+ وأجهزة أحدث.
5. **الإدخال:** تحويل اللمس إلى mouse/keyboard، وأزرار على الشاشة (overlay).
   يد البلوتوث تُمرّر كـ gamepad.
6. **الصوت:** OpenAL-Soft (مبني لأندرويد) فوق AAudio/OpenSL.
7. **دورة الحياة:** إيقاف/استئناف اللعبة، حفظ العالم، إغلاق JVM عند الخروج.

**ليش JVM داخل نفس العملية؟** لأن السطح والأحداث مرتبطة بعملية التطبيق. تشغيل
JVM كعملية منفصلة يعني تمرير `Surface` بين عمليتين، وهذا هشّ على أندرويد.
Pojav يعمل بنفس الطريقة.

**عقد الكود:** `launch::build_plan` حالياً يعيد `program + args`. المرحلة M3
ستضيف تمثيلاً أنظف:

```rust
pub struct JvmSpec {
    pub classpath: Vec<PathBuf>,
    pub main_class: String,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub natives_dir: PathBuf,
    pub heap_mb: u32,
}
```

حتى لا يحتاج جسر JNI إلى تفكيك سطر أوامر نصّي.

## 4. تدفّق التثبيت

```
GET version_manifest_v2.json
  └─ version.json (+ inheritsFrom recursively)
       ├─ downloads.client           → versions/<id>/<id>.jar
       ├─ libraries[] (rules)        → libraries/<maven path>
       ├─ assetIndex                 → assets/indexes/<id>.json
       └─ objects{}                  → assets/objects/<xx>/<sha1>
download_all(4 threads, SHA-1, .part)
  └─ natives (desktop only)          → natives/<id>/
```

مهم: لا نحمّل jar أو asset كامل في الذاكرة. الملف يُكتب وهو يُنزّل، والهاش
يُحدَّث أثناء الكتابة.

## 5. الأمان

- لا تُخزَّن كلمات المرور أبداً؛ فقط OAuth tokens.
- `auth.json` على Unix بصلاحية 0600. على أندرويد، M4 ستنقل التوكنات إلى
  **Android Keystore** (AES-GCM) بدل ملف نصي.
- Azure client id عام (public client) وليس secret.
- التحقق من SHA-1 لكل ملف قبل استخدامه (سلامة النقل، وليس توقيعاً).

## 6. مكوّنات يُعاد استخدامها (M3)

| المكوّن | الترخيص | الاستخدام |
|---|---|---|
| PojavLauncher JRE builds / `libpojavexec` | GPL-3.0 | OpenJDK لأندرويد + طريقة الإقلاع |
| Pojav LWJGL fork + GLFW stub | GPL-3.0 (غالباً) | ربط النافذة والمدخلات |
| gl4es | MIT | GL 2.1 → GLES 2/3 |
| ANGLE | BSD-3 | GLES → Vulkan |
| Zink (Mesa) | MIT | GL → Vulkan |
| OpenAL-Soft | LGPL-2.0 | الصوت |
| Slint | GPL-3.0 / commercial | الواجهة |
| rustcraft-core (هذا المشروع) | GPL-3.0 | المنطق |

بسبب GPL في الطبقة السفلية، **التطبيق النهائي GPL-3.0** ويجب نشر الكود.

## 7. مخاطر معروفة

1. **JVM + سطح NativeActivity** أعقد نقطة؛ قد تحتاج SurfaceView صغير عبر JNI
   بدل الاعتماد على نافذة NativeActivity وحدها.
2. **الأجهزة Mali** توافق gl4es أضعف من Adreno؛ لازم اختبار مصفوفة أجهزة.
3. **الذاكرة**: JVM يستهلك قبل أي مود؛ الأجهزة 2 GB قد تكتفي بـ 1.8.9 وبـ
   `-Xmx384M` فقط.
4. **التحديثات**: Mojang تغيّر صيغة JSON أحياناً؛ اختبارات `manifest` تحمينا.
5. **قانوني**: لا نوزّع أي ملف من Mojang؛ اللانشر ينزّلها للعميل.
