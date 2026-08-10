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
import dev.xstrawman.junk.download.JunkDrawer
import dev.xstrawman.junk.download.MagnetDownloader
import dev.xstrawman.junk.download.MultiConnDownloader
import dev.xstrawman.junk.download.YoutubeResolver
import dev.xstrawman.junk.download.isHttpUrl
import dev.xstrawman.junk.download.isMagnet
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class JunkViewModel(app: Application) : AndroidViewModel(app) {
    var urlText by mutableStateOf("")
    var status by mutableStateOf("INSERT COIN — paste URL / YouTube / MKV / magnet")
    var error by mutableStateOf<String?>(null)
    var progress by mutableFloatStateOf(0f)
    var speedLabel by mutableStateOf("—")
    var connLabel by mutableStateOf("0")
    var fileLabel by mutableStateOf("—")
    var phase by mutableStateOf("idle")
    var running by mutableStateOf(false)
    var success by mutableStateOf(false)
    var animTime by mutableFloatStateOf(0f)
    var savePath by mutableStateOf(JunkDrawer.absolutePathHint(app))

    private val http = MultiConnDownloader(connections = 12)
    private val magnet = MagnetDownloader(http)
    private var job: Job? = null

    fun tick(dt: Float) {
        if (running) animTime += dt
    }

    fun refreshSavePath() {
        savePath = JunkDrawer.absolutePathHint(getApplication())
    }

    fun pasteFromClipboard(text: String?) {
        val t = text?.trim().orEmpty()
        if (t.isNotEmpty()) {
            urlText = t.lines().first().trim().trim('"', '\'', '<', '>')
            status = "PASTED — hit START INJECTION → ${JunkDrawer.FOLDER_NAME}"
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
        refreshSavePath()
        status = "INJECTING → $savePath"

        val ctx = getApplication<Application>()
        job = viewModelScope.launch {
            try {
                val file = when {
                    isMagnet(url) -> {
                        status = "MAGNET → $savePath"
                        magnet.download(ctx, url, ::onProgress)
                    }
                    YoutubeResolver.looksLikeExtractable(url) -> {
                        status = "YOUTUBE/STREAM resolve…"
                        downloadExtractable(ctx, url)
                    }
                    isHttpUrl(url) -> {
                        status = "MULTI-CONN → $savePath"
                        http.download(ctx, url, onProgress = ::onProgress)
                    }
                    else -> {
                        val fixed = when {
                            url.startsWith("magnet:", true) -> url
                            url.contains("://") -> url
                            else -> "https://$url"
                        }
                        when {
                            isMagnet(fixed) -> magnet.download(ctx, fixed, ::onProgress)
                            YoutubeResolver.looksLikeExtractable(fixed) ->
                                downloadExtractable(ctx, fixed)
                            else -> http.download(ctx, fixed, onProgress = ::onProgress)
                        }
                    }
                }
                progress = 1f
                success = true
                running = false
                savePath = file.absolutePath
                status = "LEVEL CLEAR — ${file.absolutePath}"
                fileLabel = file.name
                phase = "done"
            } catch (e: Exception) {
                running = false
                success = false
                error = e.message ?: "failed"
                status = "TILT — nothing faked; see error"
                phase = "error"
            }
        }
    }

    private suspend fun downloadExtractable(ctx: Context, url: String) =
        withContext(Dispatchers.IO) {
            onProgress(
                DownloadProgress(
                    0, 0, 0.0, 0, "resolving…", "youtube-resolve",
                    savedPath = JunkDrawer.dir(ctx).absolutePath,
                ),
            )
            val resolved = try {
                YoutubeResolver.resolve(url)
            } catch (e: Exception) {
                error("Stream extract failed: ${e.message}")
            }

            val progressive = resolved.progressiveUrl
            if (!progressive.isNullOrBlank()) {
                status = "STREAM multi-conn → ${JunkDrawer.FOLDER_NAME}"
                return@withContext http.download(
                    ctx,
                    progressive,
                    preferredName = "${resolved.title}.${resolved.ext}",
                    onProgress = ::onProgress,
                )
            }

            // Audio-only extractables (music.youtube, soundcloud, bandcamp, …)
            val a = resolved.audioUrl
            if (!a.isNullOrBlank() && resolved.videoUrl.isNullOrBlank()) {
                status = "AUDIO multi-conn → ${JunkDrawer.FOLDER_NAME}"
                return@withContext http.download(
                    ctx,
                    a,
                    preferredName = "${resolved.title}.audio.m4a",
                    onProgress = ::onProgress,
                )
            }

            // Adaptive without ffmpeg on phone: best video-only (honest label)
            val v = resolved.videoUrl
            if (!v.isNullOrBlank()) {
                status = "VIDEO-ONLY multi-conn (no muxer on phone yet)"
                // If audio also exists, still pull video; note in status
                if (!a.isNullOrBlank()) {
                    status = "VIDEO-ONLY (audio URL present but phone has no ffmpeg mux)"
                }
                return@withContext http.download(
                    ctx,
                    v,
                    preferredName = "${resolved.title}.video.${resolved.ext}",
                    onProgress = ::onProgress,
                )
            }

            if (!a.isNullOrBlank()) {
                status = "AUDIO multi-conn → ${JunkDrawer.FOLDER_NAME}"
                return@withContext http.download(
                    ctx,
                    a,
                    preferredName = "${resolved.title}.audio.m4a",
                    onProgress = ::onProgress,
                )
            }

            error("No downloadable stream URL found for this link")
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
        val total = p.bytesTotal
        progress = if (total > 0L) {
            (p.bytesDone.toDouble() / total).toFloat().coerceIn(0f, 1f)
        } else {
            // Unknown size: pulse between 0.05–0.35 so syringe moves without lying at 100%
            val pulse = (0.05f + (animTime % 1.2f) / 1.2f * 0.3f)
            pulse
        }
        speedLabel = formatRate(p.bytesPerSec)
        connLabel = p.connections.toString()
        fileLabel = p.fileName
        phase = p.phase
        if (p.savedPath != null) savePath = p.savedPath
        if (p.error != null) error = p.error
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
