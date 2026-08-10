package dev.xstrawman.junk.download

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLDecoder
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.coroutineContext

/**
 * Magnet support: parse + try HTTP webseeds from .torrent metadata.
 * Full DHT peer swarm is a later libtorrent drop — we still accept magnet: links.
 */
class MagnetDownloader(
    private val http: MultiConnDownloader = MultiConnDownloader(),
) {
    private val cancel = AtomicBoolean(false)

    fun requestCancel() {
        cancel.set(true)
        http.requestCancel()
    }

    data class MagnetInfo(
        val infoHash: String?,
        val displayName: String?,
        val trackers: List<String>,
    )

    fun parseMagnet(uri: String): MagnetInfo {
        val u = uri.removePrefix("magnet:?")
        val params = u.split('&').mapNotNull {
            val i = it.indexOf('=')
            if (i < 0) null else it.substring(0, i) to it.substring(i + 1)
        }.groupBy({ it.first }, { it.second })

        val xt = params["xt"]?.firstOrNull()
        val hash = xt
            ?.substringAfter("btih:", "")
            ?.substringBefore('&')
            ?.ifBlank { null }
        val name = params["dn"]?.firstOrNull()?.let {
            URLDecoder.decode(it, Charsets.UTF_8)
        }
        val tr = params["tr"]?.map {
            URLDecoder.decode(it, Charsets.UTF_8)
        } ?: emptyList()
        return MagnetInfo(hash, name, tr)
    }

    suspend fun download(
        context: Context,
        magnet: String,
        onProgress: (DownloadProgress) -> Unit,
    ): java.io.File = withContext(Dispatchers.IO) {
        cancel.set(false)
        val info = parseMagnet(magnet)
        val label = info.displayName ?: info.infoHash?.take(12) ?: "magnet"
        val drawer = JunkDrawer.dir(context).absolutePath
        onProgress(
            DownloadProgress(0, 0, 0.0, 0, label, "magnet-resolve", savedPath = drawer),
        )

        val hash = info.infoHash ?: error("magnet missing btih info hash")

        var p = 0
        while (p < 12 && coroutineContext.isActive && !cancel.get()) {
            onProgress(
                DownloadProgress(
                    bytesDone = p.toLong(),
                    bytesTotal = 100,
                    bytesPerSec = 0.0,
                    connections = 0,
                    fileName = label,
                    phase = "magnet-resolve",
                    savedPath = drawer,
                ),
            )
            delay(60)
            p++
        }

        val torrentUrl = "https://itorrents.org/torrent/$hash.torrent"
        try {
            val webseeds = fetchWebSeeds(torrentUrl)
            if (webseeds.isNotEmpty()) {
                return@withContext http.download(
                    context,
                    webseeds.first(),
                    preferredName = info.displayName,
                    onProgress = onProgress,
                )
            }
        } catch (_: Exception) {
        }

        error(
            "Magnet $hash: no HTTP webseed found. " +
                "Save path would be $drawer. " +
                "Use a direct http(s)/.mkv link for full multi-conn speed, " +
                "or wait for native DHT/libtorrent in a later build.",
        )
    }

    private fun fetchWebSeeds(torrentUrl: String): List<String> {
        val conn = URL(torrentUrl).openConnection() as HttpURLConnection
        conn.connectTimeout = 8000
        conn.readTimeout = 8000
        conn.instanceFollowRedirects = true
        conn.requestMethod = "GET"
        return try {
            if (conn.responseCode !in 200..299) emptyList()
            else {
                val bytes = conn.inputStream.readBytes()
                extractWebSeedsFromBencode(bytes)
            }
        } catch (_: Exception) {
            emptyList()
        } finally {
            conn.disconnect()
        }
    }

    private fun extractWebSeedsFromBencode(data: ByteArray): List<String> {
        val text = data.toString(Charsets.ISO_8859_1)
        val regex = Regex("""https?://[^\s\x00-\x1f"<>]{8,}""")
        return regex.findAll(text)
            .map { it.value.trimEnd(',', ':', 'e') }
            .filter { !it.contains("itorrents", true) }
            .distinct()
            .take(5)
            .toList()
    }
}

fun isMagnet(url: String): Boolean =
    url.trim().startsWith("magnet:", ignoreCase = true)

fun isHttpUrl(url: String): Boolean {
    val u = url.trim().lowercase()
    return u.startsWith("http://") || u.startsWith("https://")
}
