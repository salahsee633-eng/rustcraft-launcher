# البناء بدون كمبيوتر (GitHub Actions)

الهدف: تبني APK من تلفونك بدون Android Studio ولا NDK ولا أي أداة ثقيلة.

## كيف تشتغل الطريقة

1. الكود يُرفع إلى مستودع GitHub (عام أو خاص).
2. GitHub يشغّل سيرفر Ubuntu x86_64 (فيه كل أدوات أندرويد).
3. الـ workflow يبني `librustcraft_ui.so` بـ cargo-ndk، ثم `gradle assembleDebug`.
4. تحمّل الـ APK من صفحة الـ Actions على تلفونك وتثبّته.

الملف الجاهز: [`.github/workflows/android-preview.yml`](../.github/workflows/android-preview.yml).

## خطوات أول مرة

### 1) حساب GitHub

سوِّ حساباً من المتصفح إن ما عندك. لا تحتاج بطاقة ولا دفع.

### 2) ارفع المشروع

أسهل طريقة من هذه البيئة (بدون كتابة توكن):

```bash
cd <مجلد المشروع>
git init -b main
git add .
git commit -m "RustCraft: core + Slint UI + Android scaffold"
gh auth login          # يطلب كود على github.com/device
gh repo create rustcraft-launcher --public --source=. --push
```

`gh auth login` لا يطلب كلمة مرورك؛ يعطيك كوداً قصيراً تفتحه في المتصفح
وتوافق. بعدها الأوامر تعمل باسم حسابك.

### 3) شغّل البناء

```bash
gh workflow run android-preview.yml
gh run watch
```

أو من المتصفح: تبويب **Actions** ← **Android preview APK** ← **Run workflow**.

### 4) نزّل الـ APK

- من المتصفح: افتح آخر تشغيل ← قسم **Artifacts** ← `rustcraft-preview-apk`.
- أو من هذه البيئة:

```bash
gh run list --workflow android-preview.yml
gh run download <RUN_ID> -n rustcraft-preview-apk -D /tmp/opencode/apk
```

ثم انسخه إلى Downloads:

```bash
python3 "$OPENCODE_CONFIG_HOME/skills/android-device/android_device.py" \
  file-save --path <مسار-الملف> --destination downloads
```

### 5) ثبّته

- افتح التنزيلات واضغط الملف.
- اسمح بـ "تثبيت تطبيقات من مصادر غير معروفة" لتطبيق الملفات/المتصفح.
- Play Protect قد يعرض تحذيراً لأن التوقيع debug؛ اختر "تثبيت على أي حال".

## ماذا تتوقع من هذه النسخة

| الشي | الحالة |
|---|---|
| واجهة RustCraft الكاملة | ✅ تعمل |
| تنزيل نسخة من الواجهة | ❌ (M2) |
| تسجيل دخول Microsoft | ❌ (M4) |
| **تشغيل Minecraft** | ❌ (M3 — جسر JVM) |

> هذه نسخة معاينة للواجهة فقط. لا تنزعج إذا ضغطت Play وظهرت رسالة أن جسر
> JVM غير منفّذ؛ هذا مقصود حتى لا يُوهم المستخدم.

## ملاحظات تقنية

- **ABI**: arm64-v8a فقط (جهازك M33). يقلل وقت البناء وحجم الـ APK.
- **NDK 27**: يدعم محاذاة 16 KB المطلوبة تدريجياً في Android 15+.
- **التوقيع**: APK بـ debug keystore. للتوزيع لاحقاً أنشئ keystore خاص.
- **الكاش**: `Swatinem/rust-cache` يخلي البناء الثاني أسرع بكثير.
- **وقت البناء المتوقع**: 8–15 دقيقة أول مرة، 3–6 دقائق بعدها.

## بدائل إذا ما تبي GitHub

1. **محلياً عبر qemu**: ممكن نظرياً لكن أدوات أندرويد x86_64 تحتاج محاكاة،
   والبناء يصير بطيئاً وهشّاً.
2. **GitLab CI**: نفس الفكرة، لكن GitHub أسهل من الجوال.
3. **خدمة بناء خارجية**: تحتاج حساباً أيضاً؛ لا ميزة إضافية.
