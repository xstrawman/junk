package dev.xstrawman.junk.download

import android.content.Context
import android.os.Build
import android.os.ParcelFileDescriptor
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.io.RandomAccessFile
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import kotlin.coroutines.coroutineContext
import kotlin.math.min

data class DownloadProgress(
    val bytesDone: Long,
    val bytesTotal: Long,
    val bytesPerSec: Double,
    val connections: Int,
    val fileName: String,
    val phase: String,
    val error: String? = null,
    val done: Boolean = false,
    /** Human path: …/Download/JUNK DRAWER/file */
    val savedPath: String? = null,
)

/**
 * aria2-style multi-connection HTTP(S) → always into public Downloads/JUNK DRAWER.
 */
class MultiConnDownloader(
    private val connections: Int = 8,
    private val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .followRedirects(true)
        .followSslRedirects(true)
        .build(),
) {
    private val cancel = AtomicBoolean(false)

    fun requestCancel() = cancel.set(true)

    suspend fun download(
        context: Context,
        url: String,
        preferredName: String? = null,
        onProgress: (DownloadProgress) -> Unit,
    ): File = withContext(Dispatchers.IO) {
        cancel.set(false)
        val drawer = JunkDrawer.dir(context)
        if (!drawer.exists() && !drawer.mkdirs()) {
            error("Cannot create ${drawer.absolutePath} — grant storage permission")
        }

        var fileName = preferredName?.takeIf { it.isNotBlank() } ?: guessFileName(url)
        fileName = fileName.replace(Regex("[\\\\/:*?\"<>|]"), "_").take(180)
        if (fileName.isBlank()) fileName = "download.bin"

        onProgress(
            DownloadProgress(0, 0, 0.0, 0, fileName, "connecting", savedPath = drawer.absolutePath),
        )

        val head = client.newCall(Request.Builder().url(url).head().build()).execute()
        var total = head.header("Content-Length")?.toLongOrNull() ?: -1L
        var acceptRanges = head.header("Accept-Ranges")?.contains("bytes", true) == true
        val finalUrl = head.request.url.toString()
        head.close()

        if (total <= 0L || !acceptRanges) {
            val probe = client.newCall(
                Request.Builder().url(url).header("Range", "bytes=0-0").build(),
            ).execute()
            acceptRanges = probe.code == 206 || probe.header("Content-Range") != null
            probe.header("Content-Range")?.let { cr ->
                total = cr.substringAfterLast('/').toLongOrNull() ?: total
            }
            if (total <= 0L) total = probe.header("Content-Length")?.toLongOrNull() ?: -1L
            probe.close()
        }

        val outFile = uniqueFile(drawer, fileName)
        val partFile = File(drawer, outFile.name + ".junk.part")

        if (total > 0 && acceptRanges && connections > 1) {
            multiDownload(finalUrl.ifBlank { url }, partFile, total, onProgress, outFile.name)
        } else {
            singleDownload(finalUrl.ifBlank { url }, partFile, total, onProgress, outFile.name)
        }

        if (cancel.get()) {
            partFile.delete()
            error("cancelled")
        }

        if (outFile.exists()) outFile.delete()
        if (!partFile.renameTo(outFile)) {
            partFile.copyTo(outFile, overwrite = true)
            partFile.delete()
        }

        // Also register in MediaStore so it shows in Downloads UI (API 29+)
        indexInMediaStore(context, outFile)

        val path = outFile.absolutePath
        onProgress(
            DownloadProgress(
                bytesDone = outFile.length(),
                bytesTotal = outFile.length(),
                bytesPerSec = 0.0,
                connections = 0,
                fileName = outFile.name,
                phase = "done",
                done = true,
                savedPath = path,
            ),
        )
        outFile
    }

    private fun indexInMediaStore(context: Context, file: File) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return
        try {
            val values = android.content.ContentValues().apply {
                put(android.provider.MediaStore.Downloads.DISPLAY_NAME, file.name)
                put(android.provider.MediaStore.Downloads.MIME_TYPE, JunkDrawer.guessMime(file.name))
                put(android.provider.MediaStore.Downloads.RELATIVE_PATH, JunkDrawer.RELATIVE_PATH)
                put(android.provider.MediaStore.Downloads.IS_PENDING, 0)
                put(android.provider.MediaStore.Downloads.SIZE, file.length())
            }
            // If file already on disk in that folder, scan is enough
            android.media.MediaScannerConnection.scanFile(
                context,
                arrayOf(file.absolutePath),
                arrayOf(JunkDrawer.guessMime(file.name)),
                null,
            )
            // Avoid duplicate MediaStore rows if scan is enough
            values.clear()
        } catch (_: Exception) {
            try {
                android.media.MediaScannerConnection.scanFile(
                    context,
                    arrayOf(file.absolutePath),
                    null,
                    null,
                )
            } catch (_: Exception) {
            }
        }
    }

    private fun uniqueFile(dir: File, name: String): File {
        val f = File(dir, name)
        if (!f.exists()) return f
        val stem = name.substringBeforeLast('.', name)
        val ext = if (name.contains('.')) name.substringAfterLast('.') else ""
        for (i in 1..999) {
            val n = if (ext.isEmpty()) "$stem-$i" else "$stem-$i.$ext"
            val c = File(dir, n)
            if (!c.exists()) return c
        }
        return File(dir, "$stem-dup.$ext")
    }

    private suspend fun multiDownload(
        url: String,
        partFile: File,
        total: Long,
        onProgress: (DownloadProgress) -> Unit,
        fileName: String,
    ) = coroutineScope {
        RandomAccessFile(partFile, "rw").use { it.setLength(total) }

        // Never more segments than bytes (avoids chunk=0 / invalid Range)
        val n = min(connections, 16).toLong().coerceAtMost(total).coerceAtLeast(1L).toInt()
        val chunk = total / n
        val done = AtomicLong(0)
        val active = AtomicLong(0)
        val start = System.nanoTime()
        val err = AtomicBoolean(false)
        var errMsg: String? = null

        val jobs = (0 until n).map { i ->
            async(Dispatchers.IO) {
                if (cancel.get()) return@async
                val from = i * chunk
                val to = if (i == n - 1) total - 1 else (i + 1) * chunk - 1
                if (from > to) return@async
                var attempt = 0
                // Segment-local credit is exception-safe: downloadRange updates this
                // even when it throws mid-stream.
                val segDone = AtomicLong(0)
                while (attempt < 4 && !cancel.get() && !err.get()) {
                    attempt++
                    try {
                        downloadRange(url, partFile, from, to, done, active, segDone)
                        return@async
                    } catch (e: Exception) {
                        // Roll back only this segment's credit, then retry from from+0
                        // by resetting segDone (file bytes may be incomplete; rewrite range).
                        val rolled = segDone.getAndSet(0)
                        if (rolled > 0) done.addAndGet(-rolled)
                        if (attempt >= 4) {
                            err.set(true)
                            errMsg = e.message
                        }
                    }
                }
            }
        }

        val ticker = async(Dispatchers.IO) {
            while (coroutineContext.isActive && !cancel.get() && done.get() < total && !err.get()) {
                val d = done.get().coerceIn(0, total)
                val elapsed = (System.nanoTime() - start) / 1e9
                val rate = if (elapsed > 0) d / elapsed else 0.0
                onProgress(
                    DownloadProgress(
                        bytesDone = d,
                        bytesTotal = total,
                        bytesPerSec = rate,
                        connections = active.get().toInt(),
                        fileName = fileName,
                        phase = "downloading",
                        savedPath = partFile.parent,
                    ),
                )
                Thread.sleep(100)
            }
        }

        jobs.awaitAll()
        ticker.cancel()

        if (cancel.get()) error("cancelled")
        if (err.get()) error(errMsg ?: "segment failed")
    }

    /**
     * Writes [from]..[to] into [partFile]. Updates [done] (global) and [segDone] (segment)
     * atomically as bytes land — so callers can roll back [segDone] on failure.
     */
    private fun downloadRange(
        url: String,
        partFile: File,
        from: Long,
        to: Long,
        done: AtomicLong,
        active: AtomicLong,
        segDone: AtomicLong,
    ) {
        active.incrementAndGet()
        try {
            val req = Request.Builder()
                .url(url)
                .header("Range", "bytes=$from-$to")
                .build()
            client.newCall(req).execute().use { resp ->
                if (!resp.isSuccessful && resp.code != 206) error("HTTP ${resp.code}")
                val body = resp.body ?: error("empty body")
                RandomAccessFile(partFile, "rw").use { raf ->
                    raf.seek(from)
                    val buf = ByteArray(256 * 1024)
                    var pos = from
                    val endInclusive = to
                    body.byteStream().use { input ->
                        while (pos <= endInclusive && !cancel.get()) {
                            val r = input.read(buf)
                            if (r < 0) break
                            val allow = (endInclusive - pos + 1).toInt().coerceAtMost(r)
                            raf.write(buf, 0, allow)
                            pos += allow
                            val add = allow.toLong()
                            done.addAndGet(add)
                            segDone.addAndGet(add)
                            if (allow < r) break
                        }
                    }
                    val need = to - from + 1
                    val got = segDone.get()
                    if (got < need && !cancel.get()) {
                        error("short read $got/$need")
                    }
                }
            }
        } finally {
            active.decrementAndGet()
        }
    }

    private fun singleDownload(
        url: String,
        partFile: File,
        knownTotal: Long,
        onProgress: (DownloadProgress) -> Unit,
        fileName: String,
    ) {
        val start = System.nanoTime()
        val req = Request.Builder().url(url).build()
        client.newCall(req).execute().use { resp ->
            if (!resp.isSuccessful) error("HTTP ${resp.code}")
            val body = resp.body ?: error("empty body")
            // 0 = unknown total → UI must NOT treat as 100%
            val total = knownTotal.takeIf { it > 0 }
                ?: body.contentLength().takeIf { it > 0 }
                ?: 0L
            var done = 0L
            partFile.outputStream().use { out ->
                body.byteStream().use { input ->
                    val buf = ByteArray(256 * 1024)
                    while (!cancel.get()) {
                        val r = input.read(buf)
                        if (r < 0) break
                        out.write(buf, 0, r)
                        done += r
                        val elapsed = (System.nanoTime() - start) / 1e9
                        val rate = if (elapsed > 0) done / elapsed else 0.0
                        onProgress(
                            DownloadProgress(
                                bytesDone = done,
                                bytesTotal = total, // 0 if unknown
                                bytesPerSec = rate,
                                connections = 1,
                                fileName = fileName,
                                phase = if (total > 0) "downloading" else "downloading-unknown-size",
                                savedPath = partFile.parent,
                            ),
                        )
                    }
                }
            }
        }
        if (cancel.get()) error("cancelled")
    }

    private fun guessFileName(url: String): String {
        val path = url.substringBefore('?').substringAfterLast('/')
        val clean = path
            .replace(Regex("[\\\\/:*?\"<>|]"), "_")
            .trim('.')
            .take(180)
        return when {
            clean.isBlank() || clean == "." || clean == ".." -> "download.bin"
            else -> clean
        }
    }
}
