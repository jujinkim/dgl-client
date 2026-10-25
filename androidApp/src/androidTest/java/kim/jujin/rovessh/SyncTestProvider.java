package kim.jujin.rovessh;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/** Test APK runs in its own process without the target's Kotlin runtime. */
public final class SyncTestProvider extends ContentProvider {
    private final Set<String> failures = ConcurrentHashMap.newKeySet();
    @Override public boolean onCreate() { return true; }
    private File file(Uri uri) {
        String id = uri.getLastPathSegment();
        if (id == null || !id.matches("[a-zA-Z0-9-]+")) throw new IllegalArgumentException();
        return new File(getContext().getCacheDir(), "sync-" + id + ".json");
    }
    @Override public Bundle call(String method, String arg, Bundle extras) {
        Uri uri = Uri.parse(arg);
        Bundle result = new Bundle();
        try {
            switch (method) {
                case "grant":
                    getContext().grantUriPermission("kim.jujin.dglc", uri, Intent.FLAG_GRANT_READ_URI_PERMISSION
                        | Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
                    break;
                case "write": Files.write(file(uri).toPath(), extras.getString("data").getBytes(StandardCharsets.UTF_8)); break;
                case "read": result.putString("data", new String(Files.readAllBytes(file(uri).toPath()), StandardCharsets.UTF_8)); break;
                case "fail": if (extras.getBoolean("enabled")) failures.add(uri.toString()); else failures.remove(uri.toString()); break;
                case "delete": file(uri).delete(); break;
                default: throw new IllegalArgumentException();
            }
        } catch (IOException e) { throw new IllegalStateException(e); }
        return result;
    }
    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (mode.contains("w") && failures.contains(uri.toString())) throw new FileNotFoundException("Injected write failure");
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.parseMode(mode));
    }
    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] args, String order) {
        MatrixCursor cursor = new MatrixCursor(new String[] { OpenableColumns.DISPLAY_NAME });
        cursor.addRow(new Object[] { "dgl-sync.json" });
        return cursor;
    }
    @Override public String getType(Uri uri) { return "application/json"; }
    @Override public Uri insert(Uri uri, ContentValues values) { return null; }
    @Override public int delete(Uri uri, String selection, String[] args) { return 0; }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] args) { return 0; }
}
