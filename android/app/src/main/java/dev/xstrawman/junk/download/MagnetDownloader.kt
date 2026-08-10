package dev.xstrawman.junk.download

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLDecoder
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.coroutineContext

/**
 * Magnet / torrent support.
 *
 * Full libtorrent is heavy for a first APK; this implementation:
 * 1. Parses magnet display name / hash for UI
 * 2. Tries public torrent→HTTP gateway mirrors when available (best-effort)
 * 3. Falls back to clear error if no HTTP path exists
 *
 * For production "real" DHT magnets, ship libtorrent4j native .so later.
 * Progress uses the same syringe UI as multi-conn HTTP.
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

    /**
     * Attempt download. Strategy:
     * - If we can resolve a direct HTTP(S) from common webtorrent / cache gateways, multi-conn it.
     * - Else report that full DHT needs the next native libtorrent build (still show arcade animation).
     */
    suspend fun download(
        magnet: String,
        destDir: File,
        onProgress: (DownloadProgress) -> Unit,
    ): File = withContext(Dispatchers.IO) {
        cancel.set(false)
        val info = parseMagnet(magnet)
        val label = info.displayName ?: info.infoHash?.take(12) ?: "magnet"
        onProgress(
            DownloadProgress(0, 0, 0.0, 0, label, "magnet-resolve"),
        )

        val hash = info.infoHash
            ?: error("magnet missing btih info hash")

        // Simulated resolve phase for UI (syringe priming)
        var p = 0
        while (p < 15 && coroutineContext.isActive && !cancel.get()) {
            onProgress(
                DownloadProgress(
                    bytesDone = p.toLong(),
                    bytesTotal = 100,
                    bytesPerSec = 0.0,
                    connections = 0,
                    fileName = label,
                    phase = "magnet-dht",
                ),
            )
            delay(80)
            p++
        }

        // Try downloading .torrent via itorrents then parse for webseed HTTP urls
        val torrentUrl = "https://itorrents.org/torrent/$hash.torrent"
        try {
            val webseeds = fetchWebSeeds(torrentUrl)
            if (webseeds.isNotEmpty()) {
                // Multi-conn first webseed URL
                return@withContext http.download(webseeds.first(), destDir, onProgress)
            }
        } catch (_: Exception) {
            // fall through
        }

        // Honest failure with arcade-friendly message — full DHT in next APK drop
        error(
            "Magnet $hash: no HTTP webseed found. " +
                "Full DHT/peer magnet support ships with libtorrent native engine next. " +
                "Direct http(s)/.mkv links still inject at full speed.",
        )
    }

    private fun fetchWebSeeds(torrentUrl: String): List<String> {
        // Minimal bencode scrape is heavy; skip deep parse for v1 APK.
        // Placeholder for future torrent file → url-list webseed extraction.
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

    /** Very small search for url-list / http(s) URLs inside torrent bytes. */
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
