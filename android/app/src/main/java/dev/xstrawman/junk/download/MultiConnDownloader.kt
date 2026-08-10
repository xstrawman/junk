package dev.xstrawman.junk.download

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
)

/**
 * aria2-style multi-connection HTTP(S) downloader for Android.
 * Supports direct files, MKV links, large ISOs, etc.
 */
class MultiConnDownloader(
    private val connections: Int = 8,
    private val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .followRedirects(true)
        .followSslRedirects(true)
        .build(),
) {
    private val cancel = AtomicBoolean(false)

    fun requestCancel() = cancel.set(true)

    suspend fun download(
        url: String,
        destDir: File,
        onProgress: (DownloadProgress) -> Unit,
    ): File = withContext(Dispatchers.IO) {
        cancel.set(false)
        destDir.mkdirs()

        val fileName = guessFileName(url)
        onProgress(
            DownloadProgress(0, 0, 0.0, 0, fileName, "connecting"),
        )

        val head = client.newCall(
            Request.Builder().url(url).head().build(),
        ).execute()

        var total = head.header("Content-Length")?.toLongOrNull() ?: -1L
        var acceptRanges = head.header("Accept-Ranges")?.contains("bytes", true) == true
        val finalUrl = head.request.url.toString()
        head.close()

        if (total <= 0L || !acceptRanges) {
            // Probe with ranged GET
            val probe = client.newCall(
                Request.Builder()
                    .url(url)
                    .header("Range", "bytes=0-0")
                    .build(),
            ).execute()
            acceptRanges = probe.code == 206 || probe.header("Content-Range") != null
            probe.header("Content-Range")?.let { cr ->
                val totalStr = cr.substringAfterLast('/')
                total = totalStr.toLongOrNull() ?: total
            }
            if (total <= 0L) {
                total = probe.header("Content-Length")?.toLongOrNull() ?: -1L
            }
            probe.close()
        }

        val outName = fileName
        val partFile = File(destDir, "$outName.junk.part")
        val finalFile = File(destDir, outName)

        if (total > 0 && acceptRanges && connections > 1) {
            multiDownload(finalUrl, partFile, total, onProgress, outName)
        } else {
            singleDownload(finalUrl.ifBlank { url }, partFile, total, onProgress, outName)
        }

        if (finalFile.exists()) finalFile.delete()
        if (!partFile.renameTo(finalFile)) {
            partFile.copyTo(finalFile, overwrite = true)
            partFile.delete()
        }
        onProgress(
            DownloadProgress(
                bytesDone = finalFile.length(),
                bytesTotal = finalFile.length(),
                bytesPerSec = 0.0,
                connections = 0,
                fileName = outName,
                phase = "done",
                done = true,
            ),
        )
        finalFile
    }

    private suspend fun multiDownload(
        url: String,
        partFile: File,
        total: Long,
        onProgress: (DownloadProgress) -> Unit,
        fileName: String,
    ) = coroutineScope {
        RandomAccessFile(partFile, "rw").use { it.setLength(total) }

        val n = min(connections, 16).coerceAtLeast(1)
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
                var attempt = 0
                while (attempt < 4 && !cancel.get() && !err.get()) {
                    attempt++
                    try {
                        downloadRange(url, partFile, from, to, done, active)
                        return@async
                    } catch (e: Exception) {
                        if (attempt >= 4) {
                            err.set(true)
                            errMsg = e.message
                        }
                    }
                }
            }
        }

        // Progress ticker
        val ticker = async(Dispatchers.IO) {
            while (coroutineContext.isActive && !cancel.get() && done.get() < total && !err.get()) {
                val d = done.get()
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

    private fun downloadRange(
        url: String,
        partFile: File,
        from: Long,
        to: Long,
        done: AtomicLong,
        active: AtomicLong,
    ) {
        active.incrementAndGet()
        try {
            val req = Request.Builder()
                .url(url)
                .header("Range", "bytes=$from-$to")
                .build()
            client.newCall(req).execute().use { resp ->
                if (!resp.isSuccessful && resp.code != 206) {
                    error("HTTP ${resp.code}")
                }
                val body = resp.body ?: error("empty body")
                RandomAccessFile(partFile, "rw").use { raf ->
                    raf.seek(from)
                    val buf = ByteArray(64 * 1024)
                    var written = 0L
                    val need = to - from + 1
                    body.byteStream().use { input ->
                        while (written < need && !cancel.get()) {
                            val r = input.read(buf)
                            if (r < 0) break
                            raf.write(buf, 0, r)
                            written += r
                            done.addAndGet(r.toLong())
                        }
                    }
                    if (written < need && !cancel.get()) {
                        error("short read $written/$need")
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
            val total = knownTotal.takeIf { it > 0 }
                ?: body.contentLength().takeIf { it > 0 } ?: -1L
            var done = 0L
            partFile.outputStream().use { out ->
                body.byteStream().use { input ->
                    val buf = ByteArray(64 * 1024)
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
                                bytesTotal = if (total > 0) total else done,
                                bytesPerSec = rate,
                                connections = 1,
                                fileName = fileName,
                                phase = "downloading",
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
        val clean = path.replace(Regex("[\\\\/:*?\"<>|]"), "_").take(180)
        return if (clean.isBlank()) "download.bin" else clean
    }
}
