package dev.xstrawman.junk

import android.app.Application
import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import dev.xstrawman.junk.download.DownloadProgress
import dev.xstrawman.junk.download.MagnetDownloader
import dev.xstrawman.junk.download.MultiConnDownloader
import dev.xstrawman.junk.download.isHttpUrl
import dev.xstrawman.junk.download.isMagnet
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import java.io.File

class JunkViewModel(app: Application) : AndroidViewModel(app) {
    var urlText by mutableStateOf("")
    var status by mutableStateOf("INSERT COIN — paste a link, MKV, or magnet")
    var error by mutableStateOf<String?>(null)
    var progress by mutableFloatStateOf(0f)
    var speedLabel by mutableStateOf("—")
    var connLabel by mutableStateOf("0")
    var fileLabel by mutableStateOf("—")
    var phase by mutableStateOf("idle")
    var running by mutableStateOf(false)
    var success by mutableStateOf(false)
    var animTime by mutableFloatStateOf(0f)

    private val http = MultiConnDownloader(connections = 8)
    private val magnet = MagnetDownloader(http)
    private var job: Job? = null

    fun tick(dt: Float) {
        if (running) animTime += dt
    }

    fun pasteFromClipboard(text: String?) {
        val t = text?.trim().orEmpty()
        if (t.isNotEmpty()) {
            urlText = t.lines().first().trim().trim('"', '\'', '<', '>')
            status = "PASTED — hit START INJECTION"
            error = null
        }
    }

    fun start() {
        val url = urlText.trim()
        if (url.isEmpty()) {
            error = "No URL — paste something first"
            return
        }
        if (running) return

        running = true
        success = false
        error = null
        progress = 0f
        phase = "start"
        status = "INJECTING…"

        val dest = downloadDir(getApplication())
        dest.mkdirs()

        job = viewModelScope.launch {
            try {
                val file = when {
                    isMagnet(url) -> {
                        status = "MAGNET — resolving peers / webseeds…"
                        magnet.download(url, dest, ::onProgress)
                    }
                    isHttpUrl(url) -> {
                        status = "MULTI-CONN HYPERSONIC…"
                        http.download(url, dest, ::onProgress)
                    }
                    else -> {
                        // bare magnet-looking or domain without scheme
                        val fixed = if (url.startsWith("magnet:", true)) url
                        else if (url.contains("://")) url
                        else "https://$url"
                        if (isMagnet(fixed)) magnet.download(fixed, dest, ::onProgress)
                        else http.download(fixed, dest, ::onProgress)
                    }
                }
                progress = 1f
                success = true
                running = false
                status = "LEVEL CLEAR — ${file.absolutePath}"
                fileLabel = file.name
                phase = "done"
            } catch (e: Exception) {
                running = false
                success = false
                error = e.message ?: "failed"
                status = "TILT"
                phase = "error"
            }
        }
    }

    fun cancel() {
        http.requestCancel()
        magnet.requestCancel()
        job?.cancel()
        running = false
        status = "CANCELLED"
        phase = "cancelled"
    }

    private fun onProgress(p: DownloadProgress) {
        val total = p.bytesTotal.coerceAtLeast(0)
        progress = if (total > 0) (p.bytesDone.toDouble() / total).toFloat().coerceIn(0f, 1f)
        else progress
        speedLabel = formatRate(p.bytesPerSec)
        connLabel = p.connections.toString()
        fileLabel = p.fileName
        phase = p.phase
        if (p.error != null) error = p.error
    }

    private fun downloadDir(ctx: Context): File {
        // App-specific movies/downloads — no special permission on modern Android
        val base = ctx.getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS)
            ?: ctx.filesDir
        return File(base, "junk")
    }

    private fun formatRate(bps: Double): String {
        if (bps <= 0 || !bps.isFinite()) return "—"
        val u = arrayOf("B/s", "KB/s", "MB/s", "GB/s")
        var v = bps
        var i = 0
        while (v >= 1024 && i < u.lastIndex) {
            v /= 1024
            i++
        }
        return "%.1f %s".format(v, u[i])
    }
}
