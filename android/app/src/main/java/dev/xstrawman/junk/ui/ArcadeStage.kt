package dev.xstrawman.junk.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Retro terminal CRT panel: long ASCII syringe is the progress bar.
 * No canvas cartoons — pure arcade terminal aesthetic (APK + future OSX GUI).
 */
@Composable
fun ArcadeStage(
    progress: Float,
    animTime: Float,
    active: Boolean,
    modifier: Modifier = Modifier,
) {
    val tick = (animTime * 4f).toInt()
    val art = AsciiSyringe.render(progress, width = 26, animTick = tick)
    val blink = if (active && (tick % 2 == 0)) "█" else " "

    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(Arcade.Panel, RoundedCornerShape(4.dp))
            .border(2.dp, Arcade.NeonCyan, RoundedCornerShape(4.dp))
            .padding(10.dp),
    ) {
        Text(
            text = "┌─ CRT CHANNEL 03 ─ JUNK INJECTOR $blink─┐",
            color = Arcade.NeonMagenta,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold,
        )
        Text(
            text = art,
            color = Arcade.NeonYellow,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp,
            lineHeight = 13.sp,
            modifier = Modifier.padding(top = 6.dp),
        )
        Text(
            text = AsciiSyringe.renderCompact(progress, 22),
            color = Arcade.NeonGreen,
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.padding(top = 6.dp),
        )
        Text(
            text = "└─ syringe = % downloaded · fluid = JUNK ─┘",
            color = Arcade.Dim,
            fontFamily = FontFamily.Monospace,
            fontSize = 10.sp,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}
