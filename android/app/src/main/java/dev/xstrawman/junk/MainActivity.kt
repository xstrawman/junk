package dev.xstrawman.junk

import android.Manifest
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import dev.xstrawman.junk.download.JunkDrawer
import dev.xstrawman.junk.ui.Arcade
import dev.xstrawman.junk.ui.ArcadeStage
import kotlinx.coroutines.delay

class MainActivity : ComponentActivity() {
    private val vm: JunkViewModel by viewModels()

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) {
        vm.refreshSavePath()
        ensureAllFilesAccessIfNeeded()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        requestStorage()
        handleIntent(intent)
        maybeClipboard()
        vm.refreshSavePath()

        setContent {
            JunkApp(
                vm = vm,
                onPasteClick = { maybeClipboard(force = true) },
                onGrantStorage = {
                    requestStorage()
                    ensureAllFilesAccessIfNeeded()
                },
            )
        }
    }

    override fun onResume() {
        super.onResume()
        vm.refreshSavePath()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun requestStorage() {
        val need = mutableListOf<String>()
        if (Build.VERSION.SDK_INT <= 28) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.WRITE_EXTERNAL_STORAGE)
                != PackageManager.PERMISSION_GRANTED
            ) {
                need += Manifest.permission.WRITE_EXTERNAL_STORAGE
            }
        } else if (Build.VERSION.SDK_INT <= 32) {
            if (ContextCompat.checkSelfPermission(this, Manifest.permission.READ_EXTERNAL_STORAGE)
                != PackageManager.PERMISSION_GRANTED
            ) {
                need += Manifest.permission.READ_EXTERNAL_STORAGE
            }
        }
        if (need.isNotEmpty()) {
            permissionLauncher.launch(need.toTypedArray())
        } else {
            ensureAllFilesAccessIfNeeded()
        }
    }

    /** Android 11+: writing freely under public Downloads/JUNK DRAWER. */
    private fun ensureAllFilesAccessIfNeeded() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (!Environment.isExternalStorageManager()) {
                try {
                    val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                        data = Uri.parse("package:$packageName")
                    }
                    startActivity(intent)
                } catch (_: Exception) {
                    try {
                        startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
                    } catch (_: Exception) {
                    }
                }
            }
        }
        JunkDrawer.dir(this)
        vm.refreshSavePath()
    }

    private fun handleIntent(intent: Intent?) {
        when (intent?.action) {
            Intent.ACTION_SEND -> {
                val t = intent.getStringExtra(Intent.EXTRA_TEXT)
                if (!t.isNullOrBlank()) vm.pasteFromClipboard(t)
            }
            Intent.ACTION_VIEW -> {
                intent.dataString?.let { vm.pasteFromClipboard(it) }
            }
        }
    }

    private fun maybeClipboard(force: Boolean = false) {
        val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = cm.primaryClip?.getItemAt(0)?.coerceToText(this)?.toString()
        if (!clip.isNullOrBlank() && (force || vm.urlText.isBlank())) {
            val t = clip.trim()
            if (t.startsWith("http", true) || t.startsWith("magnet:", true) ||
                t.contains("://") || t.endsWith(".mkv", true)
            ) {
                vm.pasteFromClipboard(t)
            } else if (force) {
                vm.pasteFromClipboard(t)
            }
        }
    }
}

@Composable
fun JunkApp(
    vm: JunkViewModel,
    onPasteClick: () -> Unit,
    onGrantStorage: () -> Unit,
) {
    val blink by rememberInfiniteTransition(label = "blink").animateFloat(
        initialValue = 0.4f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            tween(500, easing = LinearEasing),
            RepeatMode.Reverse,
        ),
        label = "blinkAnim",
    )

    LaunchedEffect(vm.running) {
        while (true) {
            delay(16)
            vm.tick(0.016f)
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    listOf(Arcade.Cabinet, Arcade.Bezel, Arcade.Cabinet),
                ),
            )
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(28.dp))
        Text(
            text = "⚡ JUNK CABINET ⚡",
            color = Arcade.NeonYellow,
            fontSize = 28.sp,
            fontWeight = FontWeight.Black,
            fontFamily = FontFamily.Monospace,
            textAlign = TextAlign.Center,
        )
        Text(
            text = "90s arcade multi-conn · YouTube · mkv · magnets",
            color = Arcade.NeonCyan.copy(alpha = blink),
            fontSize = 12.sp,
            fontFamily = FontFamily.Monospace,
        )

        Spacer(Modifier.height(8.dp))

        // Always-visible save location
        Column(
            Modifier
                .fillMaxWidth()
                .background(Arcade.Panel, RoundedCornerShape(8.dp))
                .border(2.dp, Arcade.NeonGreen, RoundedCornerShape(8.dp))
                .padding(10.dp),
        ) {
            Text(
                "JUNK DRAWER (all downloads)",
                color = Arcade.NeonGreen,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                fontSize = 12.sp,
            )
            Text(
                vm.savePath,
                color = Arcade.NeonYellow,
                fontFamily = FontFamily.Monospace,
                fontSize = 11.sp,
            )
            TextButton(onClick = onGrantStorage) {
                Text(
                    "GRANT STORAGE IF NEEDED",
                    color = Arcade.NeonCyan,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                )
            }
        }

        Spacer(Modifier.height(12.dp))

        ArcadeStage(
            progress = vm.progress,
            animTime = vm.animTime,
            active = vm.running || vm.success,
            modifier = Modifier
                .fillMaxWidth()
                .border(3.dp, Arcade.NeonMagenta, RoundedCornerShape(8.dp)),
        )

        Spacer(Modifier.height(8.dp))

        Row(
            Modifier
                .fillMaxWidth()
                .background(Arcade.Panel, RoundedCornerShape(6.dp))
                .border(2.dp, Arcade.NeonCyan, RoundedCornerShape(6.dp))
                .padding(10.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            HudCell("SPEED", vm.speedLabel)
            HudCell("CONN", vm.connLabel)
            HudCell("FILE", vm.fileLabel.take(14))
            HudCell("PHASE", vm.phase.take(10))
        }

        Spacer(Modifier.height(12.dp))

        OutlinedTextField(
            value = vm.urlText,
            onValueChange = { vm.urlText = it },
            modifier = Modifier.fillMaxWidth(),
            textStyle = TextStyle(
                color = Arcade.NeonYellow,
                fontFamily = FontFamily.Monospace,
                fontSize = 14.sp,
            ),
            placeholder = {
                Text(
                    "YouTube · https://…/file.iso · .mkv · magnet:?",
                    color = Arcade.Dim,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 13.sp,
                )
            },
            singleLine = false,
            maxLines = 3,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { vm.start() }),
            colors = OutlinedTextFieldDefaults.colors(
                focusedBorderColor = Arcade.NeonMagenta,
                unfocusedBorderColor = Arcade.NeonCyan,
                cursorColor = Arcade.NeonGreen,
                focusedContainerColor = Arcade.Panel,
                unfocusedContainerColor = Arcade.Panel,
            ),
            shape = RoundedCornerShape(8.dp),
        )

        Spacer(Modifier.height(10.dp))

        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton(onClick = onPasteClick, modifier = Modifier.weight(1f)) {
                Text("📋 PASTE", color = Arcade.NeonCyan, fontFamily = FontFamily.Monospace)
            }
            Button(
                onClick = { if (vm.running) vm.cancel() else vm.start() },
                modifier = Modifier.weight(2f),
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (vm.running) Arcade.NeonRed else Arcade.NeonMagenta,
                    contentColor = Arcade.Cabinet,
                ),
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(
                    if (vm.running) "■ CANCEL" else "▶ START INJECTION",
                    fontWeight = FontWeight.Black,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 14.sp,
                )
            }
        }

        Spacer(Modifier.height(12.dp))

        Text(
            text = vm.status,
            color = when {
                vm.error != null -> Arcade.NeonRed
                vm.success -> Arcade.NeonGreen
                else -> Arcade.NeonYellow
            },
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            textAlign = TextAlign.Center,
            modifier = Modifier.fillMaxWidth(),
        )
        vm.error?.let {
            Text(
                text = it,
                color = Arcade.NeonRed,
                fontFamily = FontFamily.Monospace,
                fontSize = 11.sp,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 6.dp),
            )
        }

        Spacer(Modifier.height(24.dp))
    }
}

@Composable
private fun HudCell(label: String, value: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(label, color = Arcade.Dim, fontSize = 9.sp, fontFamily = FontFamily.Monospace)
        Spacer(Modifier.width(4.dp))
        Text(
            value,
            color = Arcade.NeonGreen,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
            fontFamily = FontFamily.Monospace,
            maxLines = 1,
        )
    }
}
