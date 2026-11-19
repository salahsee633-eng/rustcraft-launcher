package org.rustcraft;

import android.app.NativeActivity;
import android.os.Bundle;

/** Thin Java shell around NativeActivity so startup failures become visible. */
public class LauncherActivity extends NativeActivity {
    private static native void nativeRegisterDiagClass(Class<?> diagClass);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        Thread.UncaughtExceptionHandler previous = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((thread, error) -> {
            Diag.logThrowable(this, "Java uncaught on " + thread.getName(), error);
            if (previous != null) previous.uncaughtException(thread, error);
        });
        Diag.log(this, "Java onCreate start");

        try {
            System.loadLibrary("rustcraft_ui");
            Diag.log(this, "Native library loaded");
        } catch (Throwable error) {
            Diag.logThrowable(this, "Native library load failed", error);
        }

        try {
            nativeRegisterDiagClass(Diag.class);
            Diag.log(this, "Diag class registered");
        } catch (Throwable error) {
            Diag.logThrowable(this, "Diag class registration failed", error);
        }

        try {
            super.onCreate(savedInstanceState);
            Diag.log(this, "NativeActivity onCreate returned");
        } catch (Throwable error) {
            Diag.logThrowable(this, "NativeActivity onCreate failed", error);
            throw error;
        }
    }

    @Override
    protected void onStart() {
        Diag.log(this, "Java onStart");
        super.onStart();
    }

    @Override
    protected void onResume() {
        Diag.log(this, "Java onResume");
        super.onResume();
    }
}
