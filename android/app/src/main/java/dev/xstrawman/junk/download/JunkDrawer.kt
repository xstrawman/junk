package dev.xstrawman.junk.download

import android.content.ContentValues
import android.content.Context
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File

/**
 * All user downloads land in public Downloads/JUNK DRAWER.
 * Visible in Files app, Gallery/Downloads UI, USB, etc.
 */
object JunkDrawer {
    const val FOLDER_NAME = "JUNK DRAWER"
    /** Relative path for MediaStore (no trailing slash). */
    const val RELATIVE_PATH = "Download/$FOLDER_NAME"

    fun dir(context: Context): File {
        val base = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
        val junk = File(base, FOLDER_NAME)
        if (!junk.exists()) junk.mkdirs()
        return junk
    }

    fun absolutePathHint(context: Context): String {
        return dir(context).absolutePath
    }

    /**
     * Create a pending MediaStore entry under Downloads/JUNK DRAWER (API 29+).
     * Returns content Uri string for later publish, or null to use plain File.
     */
    fun insertPending(context: Context, fileName: String, mime: String): android.net.Uri? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return null
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, fileName)
            put(MediaStore.Downloads.MIME_TYPE, mime)
            put(MediaStore.Downloads.RELATIVE_PATH, RELATIVE_PATH)
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        return context.contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
    }

    fun publish(context: Context, uri: android.net.Uri) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return
        val values = ContentValues().apply {
            put(MediaStore.Downloads.IS_PENDING, 0)
        }
        context.contentResolver.update(uri, values, null, null)
    }

    fun guessMime(name: String): String {
        val n = name.lowercase()
        return when {
            n.endsWith(".mp4") -> "video/mp4"
            n.endsWith(".mkv") -> "video/x-matroska"
            n.endsWith(".webm") -> "video/webm"
            n.endsWith(".mp3") -> "audio/mpeg"
            n.endsWith(".iso") -> "application/x-iso9660-image"
            n.endsWith(".zip") -> "application/zip"
            n.endsWith(".pdf") -> "application/pdf"
            else -> "application/octet-stream"
        }
    }
}
