package dev.xstrawman.junk.download

import okhttp3.OkHttpClient
import okhttp3.RequestBody.Companion.toRequestBody
import org.schabi.newpipe.extractor.NewPipe
import org.schabi.newpipe.extractor.ServiceList
import org.schabi.newpipe.extractor.downloader.Downloader
import org.schabi.newpipe.extractor.downloader.Request
import org.schabi.newpipe.extractor.downloader.Response
import org.schabi.newpipe.extractor.exceptions.ReCaptchaException
import org.schabi.newpipe.extractor.stream.StreamInfo
import org.schabi.newpipe.extractor.stream.VideoStream
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

data class ResolvedStream(
    val title: String,
    val videoUrl: String?,
    val audioUrl: String?,
    /** progressive single-file URL when video+audio already muxed */
    val progressiveUrl: String?,
    val ext: String,
)

/**
 * NewPipe Extractor based stream resolver (YouTube and other NewPipe services).
 * Does not run yt-dlp — pure JVM, works on Android.
 */
object YoutubeResolver {
    private val ready = AtomicBoolean(false)
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(45, TimeUnit.SECONDS)
        .followRedirects(true)
        .build()

    fun initIfNeeded() {
        if (ready.get()) return
        synchronized(this) {
            if (ready.get()) return
            NewPipe.init(OkHttpDownloader(client))
            ready.set(true)
        }
    }

    fun looksLikeExtractable(url: String): Boolean {
        val u = url.lowercase()
        return listOf(
            "youtube.com", "youtu.be", "youtube-nocookie.com",
            "music.youtube.com",
            "vimeo.com", "bandcamp.com", "soundcloud.com",
            "bilibili.com", "twitch.tv", "peertube",
        ).any { u.contains(it) }
    }

    /**
     * Prefer progressive MP4 ≤720p; else best video-only + best audio for client merge note.
     * Android APK downloads progressive when possible (no ffmpeg required).
     */
    fun resolve(url: String): ResolvedStream {
        initIfNeeded()
        val info = StreamInfo.getInfo(url)
        val title = info.name?.ifBlank { "video" } ?: "video"
        val safeTitle = title.replace(Regex("[\\\\/:*?\"<>|]"), "-").trim().ifBlank { "video" }

        // Progressive (video+audio)
        @Suppress("DEPRECATION")
        val progressive = info.videoStreams
            ?.filter { it.isVideoOnly.not() }
            ?.sortedByDescending { it.height }
            ?.firstOrNull { it.height in 1..1080 }
            ?: info.videoStreams?.filter { !it.isVideoOnly }?.maxByOrNull { it.height }

        if (progressive != null && !progressive.content.isNullOrBlank()) {
            val ext = progressive.format?.suffix ?: "mp4"
            return ResolvedStream(
                title = safeTitle,
                videoUrl = null,
                audioUrl = null,
                progressiveUrl = progressive.content,
                ext = ext,
            )
        }

        // Adaptive: best video + best audio
        val videoOnly = info.videoOnlyStreams
            ?.sortedByDescending { it.height }
            ?.firstOrNull { it.height <= 1080 }
            ?: info.videoOnlyStreams?.maxByOrNull { it.height }

        val audio = info.audioStreams
            ?.maxByOrNull { it.averageBitrate }

        return ResolvedStream(
            title = safeTitle,
            videoUrl = videoOnly?.content,
            audioUrl = audio?.content,
            progressiveUrl = null,
            ext = videoOnly?.format?.suffix ?: "mp4",
        )
    }
}

/** OkHttp bridge for NewPipe Extractor. */
private class OkHttpDownloader(
    private val client: OkHttpClient,
) : Downloader() {
    override fun execute(request: Request): Response {
        val builder = okhttp3.Request.Builder()
            .url(request.url())
            .method(
                request.httpMethod(),
                request.dataToSend()?.toRequestBody(null),
            )
        request.headers().forEach { (k, values) ->
            values.forEach { v -> builder.addHeader(k, v) }
        }
        if (request.headers()["User-Agent"].isNullOrEmpty()) {
            builder.header(
                "User-Agent",
                "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120.0.0.0 Mobile Safari/537.36",
            )
        }
        client.newCall(builder.build()).execute().use { resp ->
            if (resp.code == 429) {
                throw ReCaptchaException("reCAPTCHA / rate limit", request.url())
            }
            val body = resp.body?.string() ?: ""
            val headers = LinkedHashMap<String, List<String>>()
            resp.headers.forEach { (name, value) ->
                headers[name] = listOf(value)
            }
            return Response(resp.code, resp.message, headers, body, request.url())
        }
    }
}
