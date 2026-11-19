package org.rustcraft;

import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.MediaStore;

import java.io.File;
import java.io.FileWriter;
import java.io.OutputStream;
import java.io.PrintWriter;
import java.io.StringWriter;

/**
 * Tiny diagnostics bridge: keeps one cumulative startup log and writes it to
 * the public Downloads folder so it can be shared without adb or root.
 */
public final class Diag {
    private static final String NAME = "rustcraft-startup.log";
    private static final StringBuilder BUFFER = new StringBuilder();

    private Diag() {}

    public static synchronized void log(Context context, String line) {
        BUFFER.append(line);
        if (BUFFER.length() == 0 || BUFFER.charAt(BUFFER.length() - 1) != '\n') {
            BUFFER.append('\n');
        }
        write(context);
    }

    public static synchronized void logThrowable(Context context, String label, Throwable error) {
        StringWriter sw = new StringWriter();
        error.printStackTrace(new PrintWriter(sw));
        log(context, label + ": " + error + "\n" + sw);
    }

    private static synchronized void write(Context context) {
        if (context == null) return;
        String text = BUFFER.toString();
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                Uri collection = MediaStore.Downloads.EXTERNAL_CONTENT_URI;
                String[] projection = { MediaStore.MediaColumns._ID };
                String selection = MediaStore.MediaColumns.DISPLAY_NAME + "=?";
                Cursor cursor = context.getContentResolver().query(
                        collection, projection, selection, new String[] { NAME }, null);
                if (cursor != null) {
                    try {
                        if (cursor.moveToFirst()) {
                            long id = cursor.getLong(0);
                            context.getContentResolver().delete(
                                    collection,
                                    MediaStore.MediaColumns._ID + "=?",
                                    new String[] { String.valueOf(id) });
                        }
                    } finally {
                        cursor.close();
                    }
                }

                ContentValues values = new ContentValues();
                values.put(MediaStore.MediaColumns.DISPLAY_NAME, NAME);
                values.put(MediaStore.MediaColumns.MIME_TYPE, "text/plain");
                values.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS);
                Uri uri = context.getContentResolver().insert(collection, values);
                if (uri == null) return;
                try (OutputStream out = context.getContentResolver().openOutputStream(uri)) {
                    out.write(text.getBytes("UTF-8"));
                }
            } else {
                File dir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS);
                //noinspection ResultOfMethodCallIgnored
                dir.mkdirs();
                try (FileWriter writer = new FileWriter(new File(dir, NAME), false)) {
                    writer.write(text);
                }
            }
        } catch (Throwable ignored) {
            // Never let diagnostics kill the app.
        }
    }
}
