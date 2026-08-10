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
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
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
import dev.xstrawman.junk.ui.JunkDrawerPanel
import dev.xstrawman.junk.ui.openJunkDrawerInFileManager
import kotlinx.coroutines.delay

/**
 * APK "GUI" = retro arcade stylized **terminal screen**.
 * Syringe ASCII = progress. JUNK DRAWER = ls + open system Files.
 */
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
            TerminalCabinet(
                vm = vm,
                onPasteClick = { maybeClipboard(force = true) },
                onGrantStorage = {
                    requestStorage()
                    ensureAllFilesAccessIfNeeded()
                },
                onOpenDrawerExternal = {
                    val ok = openJunkDrawerInFileManager(this)
                    if (!ok) {
                        Toast.makeText(
                            this,
                            "Open Files → Downloads → JUNK DRAWER",
                            Toast.LENGTH_LONG,
                        ).show()
                    }
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

    private fun ensureAllFilesAccessIfNeeded() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            if (!Environment.isExternalStorageManager()) {
                try {
                    startActivity(
                        Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                            data = Uri.parse("package:$packageName")
                        },
                    )
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
            Intent.ACTION_VIEW -> intent.dataString?.let { vm.pasteFromClipboard(it) }
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
fun TerminalCabinet(
    vm: JunkViewModel,
    onPasteClick: () -> Unit,
    onGrantStorage: () -> Unit,
    onOpenDrawerExternal: () -> Unit,
) {
    var drawerRefresh by remember { mutableIntStateOf(0) }
    var showDrawer by remember { mutableIntStateOf(1) } // 1 = show terminal ls by default

    LaunchedEffect(vm.running, vm.success) {
        while (true) {
            delay(16)
            vm.tick(0.016f)
        }
    }
    LaunchedEffect(vm.success, vm.phase) {
        if (vm.success || vm.phase == "done") drawerRefresh++
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(listOf(Arcade.Cabinet, Arcade.Bezel, Arcade.Cabinet)),
            )
            .verticalScroll(rememberScrollState())
            .padding(12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(24.dp))

        // Terminal chrome
        Text(
            "╔══════════════════════════════════════╗",
            color = Arcade.NeonCyan,
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp,
        )
        Text(
            "║  JUNK OS v0.2  ·  ARCADE TERMINAL    ║",
            color = Arcade.NeonYellow,
            fontFamily = FontFamily.Monospace,
            fontWeight = FontWeight.Black,
            fontSize = 13.sp,
        )
        Text(
            "║  syringe = progress · drawer = /dl   ║",
            color = Arcade.NeonMagenta,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
        )
        Text(
            "╚══════════════════════════════════════╝",
            color = Arcade.NeonCyan,
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp,
        )

        Spacer(Modifier.height(8.dp))

        // Path line
        Text(
            "\$ mkdir -p ~/Download/JUNK\\ DRAWER",
            color = Arcade.Dim,
            fontFamily = FontFamily.Monospace,
            fontSize = 10.sp,
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            "\$ pwd → ${vm.savePath}",
            color = Arcade.NeonGreen,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            modifier = Modifier
                .fillMaxWidth()
                .background(Arcade.Panel, RoundedCornerShape(2.dp))
                .border(1.dp, Arcade.NeonGreen, RoundedCornerShape(2.dp))
                .padding(6.dp),
        )

        Spacer(Modifier.height(8.dp))

        // THE syringe (progress)
        ArcadeStage(
            progress = vm.progress,
            animTime = vm.animTime,
            active = vm.running || vm.success,
            modifier = Modifier.fillMaxWidth(),
        )

        Spacer(Modifier.height(8.dp))

        // HUD row terminal style
        Text(
            "SPEED ${vm.speedLabel}  CONN ${vm.connLabel}  FILE ${vm.fileLabel.take(16)}  ${vm.phase}",
            color = Arcade.NeonCyan,
            fontFamily = FontFamily.Monospace,
            fontSize = 10.sp,
            modifier = Modifier.fillMaxWidth(),
        )

        Spacer(Modifier.height(8.dp))

        // URL field looks like shell prompt
        Text(
            "\$ junk --inject",
            color = Arcade.NeonMagenta,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = vm.urlText,
            onValueChange = { vm.urlText = it },
            modifier = Modifier.fillMaxWidth(),
            textStyle = TextStyle(
                color = Arcade.NeonYellow,
                fontFamily = FontFamily.Monospace,
                fontSize = 13.sp,
            ),
            placeholder = {
                Text(
                    "https://…  |  .mkv  |  magnet:?xt=…  |  youtube",
                    color = Arcade.Dim,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                )
            },
            singleLine = false,
            maxLines = 3,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Go),
            keyboardActions = KeyboardActions(onGo = { vm.start() }),
            colors = OutlinedTextFieldDefaults.colors(
                focusedBorderColor = Arcade.NeonYellow,
                unfocusedBorderColor = Arcade.Dim,
                cursorColor = Arcade.NeonGreen,
                focusedContainerColor = Arcade.Panel,
                unfocusedContainerColor = Arcade.Panel,
            ),
            shape = RoundedCornerShape(2.dp),
        )

        Spacer(Modifier.height(8.dp))

        // Action row: PASTE | INJECT | JUNK DRAWER
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            TextButton(onClick = onPasteClick, modifier = Modifier.weight(1f)) {
                Text("PASTE", color = Arcade.NeonCyan, fontFamily = FontFamily.Monospace, fontSize = 12.sp)
            }
            Button(
                onClick = {
                    if (vm.running) vm.cancel() else vm.start()
                    drawerRefresh++
                },
                modifier = Modifier.weight(1.4f),
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (vm.running) Arcade.NeonRed else Arcade.NeonMagenta,
                    contentColor = Arcade.Cabinet,
                ),
                shape = RoundedCornerShape(2.dp),
            ) {
                Text(
                    if (vm.running) "CANCEL" else "INJECT",
                    fontWeight = FontWeight.Black,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 13.sp,
                )
            }
            Button(
                onClick = {
                    showDrawer = 1
                    drawerRefresh++
                    onOpenDrawerExternal()
                },
                modifier = Modifier.weight(1.3f),
                colors = ButtonDefaults.buttonColors(
                    containerColor = Arcade.NeonGreen,
                    contentColor = Arcade.Cabinet,
                ),
                shape = RoundedCornerShape(2.dp),
            ) {
                Text(
                    "JUNK DRAWER",
                    fontWeight = FontWeight.Black,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                )
            }
        }

        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            TextButton(onClick = onGrantStorage) {
                Text("chmod storage", color = Arcade.Dim, fontFamily = FontFamily.Monospace, fontSize = 10.sp)
            }
            TextButton(onClick = {
                showDrawer = if (showDrawer == 1) 0 else 1
                drawerRefresh++
            }) {
                Text(
                    if (showDrawer == 1) "hide ls" else "show ls",
                    color = Arcade.NeonGreen,
                    fontFamily = FontFamily.Monospace,
                    fontSize = 10.sp,
                )
            }
        }

        if (showDrawer == 1) {
            Spacer(Modifier.height(6.dp))
            JunkDrawerPanel(
                refreshKey = drawerRefresh,
                onOpenExternal = onOpenDrawerExternal,
            )
        }

        Spacer(Modifier.height(10.dp))

        Text(
            text = "> ${vm.status}",
            color = when {
                vm.error != null -> Arcade.NeonRed
                vm.success -> Arcade.NeonGreen
                else -> Arcade.NeonYellow
            },
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp,
            textAlign = TextAlign.Start,
            modifier = Modifier.fillMaxWidth(),
        )
        vm.error?.let {
            Text(
                text = "! $it",
                color = Arcade.NeonRed,
                fontFamily = FontFamily.Monospace,
                fontSize = 11.sp,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Spacer(Modifier.height(16.dp))
        Text(
            "GUI = arcade terminal · syringe = % · drawer = Downloads/JUNK DRAWER",
            color = Arcade.Dim,
            fontFamily = FontFamily.Monospace,
            fontSize = 9.sp,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(20.dp))
    }
}
