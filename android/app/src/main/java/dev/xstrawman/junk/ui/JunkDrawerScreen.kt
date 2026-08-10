package dev.xstrawman.junk.ui

import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.xstrawman.junk.download.JunkDrawer
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

data class DrawerEntry(
    val name: String,
    val sizeLabel: String,
    val timeLabel: String,
)

@Composable
fun JunkDrawerPanel(
    refreshKey: Int,
    modifier: Modifier = Modifier,
    onOpenExternal: () -> Unit,
) {
    val context = LocalContext.current
    var entries by remember { mutableStateOf<List<DrawerEntry>>(emptyList()) }
    var error by remember { mutableStateOf<String?>(null) }
    val dir = remember { JunkDrawer.dir(context) }

    fun reload() {
        try {
            if (!dir.exists()) dir.mkdirs()
            val fmt = SimpleDateFormat("MM-dd HH:mm", Locale.US)
            entries = dir.listFiles()
                ?.filter { it.isFile && !it.name.endsWith(".junk.part") }
                ?.sortedByDescending { it.lastModified() }
                ?.map { f ->
                    DrawerEntry(
                        name = f.name,
                        sizeLabel = humanSize(f.length()),
                        timeLabel = fmt.format(Date(f.lastModified())),
                    )
                }
                ?: emptyList()
            error = null
        } catch (e: Exception) {
            error = e.message
            entries = emptyList()
        }
    }

    LaunchedEffect(refreshKey) { reload() }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 120.dp, max = 300.dp)
            .background(Arcade.Cabinet, RoundedCornerShape(4.dp))
            .border(2.dp, Arcade.NeonGreen, RoundedCornerShape(4.dp))
            .padding(8.dp),
    ) {
        Text(
            "╔═ JUNK DRAWER ─ \$ ls -lh ═╗",
            color = Arcade.NeonGreen,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Bold,
            fontSize = 12.sp,
        )
        Text(
            dir.absolutePath,
            color = Arcade.NeonYellow,
            fontFamily = FontFamily.Monospace,
            fontSize = 10.sp,
        )
        Row {
            TextButton(onClick = { reload() }) {
                Text("REFRESH", color = Arcade.NeonCyan, fontFamily = FontFamily.Monospace, fontSize = 11.sp)
            }
            TextButton(onClick = onOpenExternal) {
                Text("OPEN IN FILES ▶", color = Arcade.NeonMagenta, fontFamily = FontFamily.Monospace, fontSize = 11.sp)
            }
        }
        Text(
            "─ name ──────────────── size ── when ─",
            color = Arcade.Dim,
            fontFamily = FontFamily.Monospace,
            fontSize = 10.sp,
        )
        Column(
            Modifier
                .heightIn(max = 180.dp)
                .verticalScroll(rememberScrollState()),
        ) {
            when {
                error != null -> Text(
                    "! $error",
                    color = Arcade.NeonRed,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                )
                entries.isEmpty() -> Text(
                    "> (empty — inject something)",
                    color = Arcade.Dim,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                )
                else -> entries.forEach { e ->
                    Text(
                        text = String.format(
                            Locale.US,
                            "> %-18s %8s  %s",
                            e.name.take(18),
                            e.sizeLabel,
                            e.timeLabel,
                        ),
                        color = Arcade.NeonCyan,
                        fontFamily = FontFamily.Monospace,
                        fontSize = 11.sp,
                        modifier = Modifier.padding(vertical = 2.dp),
                    )
                }
            }
        }
        Spacer(Modifier.height(4.dp))
        Text(
            "╚═ ${entries.size} file(s) ═╝",
            color = Arcade.Dim,
            fontFamily = FontFamily.Monospace,
            fontSize = 10.sp,
        )
    }
}

/**
 * Prompt the system file manager to open Downloads/JUNK DRAWER when possible.
 */
fun openJunkDrawerInFileManager(context: android.content.Context): Boolean {
    val dir = JunkDrawer.dir(context)
    dir.mkdirs()

    val candidates = listOf(
        // DocumentsUI path for primary storage Download/JUNK DRAWER
        Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(
                Uri.parse(
                    "content://com.android.externalstorage.documents/document/" +
                        "primary%3ADownload%2FJUNK%20DRAWER",
                ),
                DocumentsContract.Document.MIME_TYPE_DIR,
            )
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        },
        Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(
                DocumentsContract.buildDocumentUri(
                    "com.android.externalstorage.documents",
                    "primary:Download/${JunkDrawer.FOLDER_NAME}",
                ),
                DocumentsContract.Document.MIME_TYPE_DIR,
            )
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        },
        // Downloads root
        Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(
                Uri.parse(
                    "content://com.android.externalstorage.documents/document/primary%3ADownload",
                ),
                DocumentsContract.Document.MIME_TYPE_DIR,
            )
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        },
    )

    for (intent in candidates) {
        try {
            context.startActivity(intent)
            return true
        } catch (_: Exception) {
        }
    }

    // OEM file managers sometimes accept file://
    return try {
        @Suppress("DEPRECATION")
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(Uri.fromFile(dir), "resource/folder")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        context.startActivity(intent)
        true
    } catch (_: Exception) {
        try {
            val dl = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
            @Suppress("DEPRECATION")
            context.startActivity(
                Intent(Intent.ACTION_VIEW).apply {
                    setDataAndType(Uri.fromFile(dl), "resource/folder")
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                },
            )
            true
        } catch (_: Exception) {
            false
        }
    }
}

private fun humanSize(n: Long): String {
    if (n < 1024) return "${n}B"
    val kb = n / 1024.0
    if (kb < 1024) return String.format(Locale.US, "%.0fK", kb)
    val mb = kb / 1024.0
    if (mb < 1024) return String.format(Locale.US, "%.1fM", mb)
    return String.format(Locale.US, "%.2fG", mb / 1024.0)
}
