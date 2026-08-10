package dev.xstrawman.junk.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import kotlin.math.sin

/**
 * 90s cabinet stage: giant syringe injects neon fluid into a line of
 * original yellow cartoon noggins (arcade vibe — not anyone's IP).
 * Progress 0f..1f empties the barrel and "injects" characters left→right.
 */
@Composable
fun ArcadeStage(
    progress: Float,
    animTime: Float,
    active: Boolean,
    modifier: Modifier = Modifier,
) {
    val p = progress.coerceIn(0f, 1f)
    val hairColors = remember {
        listOf(Arcade.HairBlue, Arcade.HairPink, Arcade.HairGreen, Arcade.NeonMagenta, Arcade.NeonCyan)
    }

    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(220.dp),
    ) {
        val w = size.width
        val h = size.height

        // Cabinet scanlines backdrop
        drawRect(Arcade.Panel)
        var y = 0f
        while (y < h) {
            drawRect(
                color = Color(0x12000000),
                topLeft = Offset(0f, y),
                size = Size(w, 2f),
            )
            y += 4f
        }

        // Pixel bezel
        drawRoundRect(
            color = Arcade.Bezel,
            topLeft = Offset(4f, 4f),
            size = Size(w - 8f, h - 8f),
            cornerRadius = CornerRadius(12f, 12f),
            style = Stroke(width = 6f),
        )

        // --- Syringe (left) ---
        val sx = w * 0.08f
        val sy = h * 0.18f
        val sw = w * 0.14f
        val sh = h * 0.55f

        // plunger
        val plungerBob = if (active) sin(animTime * 6f) * 3f else 0f
        drawRect(Arcade.NeonCyan, Offset(sx + sw * 0.25f, sy - 18f + plungerBob), Size(sw * 0.5f, 16f))
        drawRect(Arcade.Dim, Offset(sx + sw * 0.35f, sy - 4f + plungerBob), Size(sw * 0.3f, 10f))

        // barrel
        drawRoundRect(
            color = Arcade.Ink,
            topLeft = Offset(sx, sy),
            size = Size(sw, sh),
            cornerRadius = CornerRadius(6f, 6f),
            style = Stroke(width = 4f),
        )
        // fluid remaining (empties as progress rises)
        val fluidFrac = 1f - p
        val fluidH = sh * 0.85f * fluidFrac
        val fluidTop = sy + sh * 0.1f + (sh * 0.85f - fluidH)
        if (fluidH > 2f) {
            drawRect(
                color = Arcade.Fluid.copy(alpha = 0.9f),
                topLeft = Offset(sx + 6f, fluidTop),
                size = Size(sw - 12f, fluidH),
            )
        }
        // tick marks
        for (i in 1..4) {
            val ty = sy + sh * i / 5f
            drawLine(Arcade.Dim, Offset(sx + 4f, ty), Offset(sx + sw * 0.3f, ty), strokeWidth = 2f)
        }

        // needle
        val needleX = sx + sw
        val needleY = sy + sh * 0.55f
        drawLine(
            color = Arcade.NeonYellow,
            start = Offset(needleX, needleY),
            end = Offset(needleX + w * 0.08f, needleY),
            strokeWidth = 5f,
            cap = StrokeCap.Round,
        )
        // drip pulse
        if (active && p in 0.01f..0.99f) {
            val drip = ((animTime * 4f) % 1f)
            drawCircle(
                Arcade.Fluid,
                radius = 4f + drip * 3f,
                center = Offset(needleX + w * 0.08f + drip * 20f, needleY),
            )
        }

        // --- Character line (brains get "loaded") ---
        val count = 6
        val startX = w * 0.38f
        val spacing = (w * 0.58f) / count
        val baseY = h * 0.62f
        val injected = (p * count).toInt().coerceIn(0, count)

        for (i in 0 until count) {
            val cx = startX + spacing * i + spacing * 0.35f
            val bounce = if (active && i == injected.coerceAtMost(count - 1) && p < 1f) {
                sin(animTime * 10f + i) * 4f
            } else {
                0f
            }
            val loaded = i < injected || (i == injected && p >= 1f) || (p >= (i + 1f) / count)
            val hair = hairColors[i % hairColors.size]
            drawCartoonHead(
                cx = cx,
                cy = baseY + bounce,
                scale = spacing * 0.42f,
                hair = hair,
                loaded = loaded,
                flash = active && loaded && sin(animTime * 12f + i) > 0.3f,
            )
        }

        // Score text strip
        val pct = (p * 100).toInt()
        // drawn outside via Compose text usually; here a bar under feet
        drawRoundRect(
            color = Arcade.Ink,
            topLeft = Offset(w * 0.38f, h * 0.88f),
            size = Size(w * 0.55f, h * 0.08f),
            cornerRadius = CornerRadius(4f, 4f),
        )
        drawRoundRect(
            color = Arcade.NeonGreen,
            topLeft = Offset(w * 0.38f, h * 0.88f),
            size = Size(w * 0.55f * p, h * 0.08f),
            cornerRadius = CornerRadius(4f, 4f),
        )
    }
}

private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawCartoonHead(
    cx: Float,
    cy: Float,
    scale: Float,
    hair: Color,
    loaded: Boolean,
    flash: Boolean,
) {
    val r = scale
    // body stub
    drawRoundRect(
        color = Color(0xFFEE4444),
        topLeft = Offset(cx - r * 0.55f, cy + r * 0.55f),
        size = Size(r * 1.1f, r * 0.7f),
        cornerRadius = CornerRadius(8f, 8f),
    )
    // head
    drawCircle(Arcade.SkinYellow, radius = r, center = Offset(cx, cy))
    drawCircle(Arcade.SkinShadow, radius = r, center = Offset(cx, cy), style = Stroke(3f))
    // hair tuft (spiky arcade)
    val hairPath = Path().apply {
        moveTo(cx - r * 0.7f, cy - r * 0.3f)
        lineTo(cx - r * 0.4f, cy - r * 1.15f)
        lineTo(cx - r * 0.1f, cy - r * 0.5f)
        lineTo(cx + r * 0.15f, cy - r * 1.2f)
        lineTo(cx + r * 0.4f, cy - r * 0.45f)
        lineTo(cx + r * 0.75f, cy - r * 1.05f)
        lineTo(cx + r * 0.65f, cy - r * 0.2f)
        close()
    }
    drawPath(hairPath, hair)

    // eyes
    val eyeY = cy - r * 0.1f
    drawCircle(Color.White, r * 0.22f, Offset(cx - r * 0.28f, eyeY))
    drawCircle(Color.White, r * 0.22f, Offset(cx + r * 0.28f, eyeY))
    val pupil = if (loaded) Arcade.NeonMagenta else Arcade.Ink
    drawCircle(pupil, r * 0.1f, Offset(cx - r * 0.22f, eyeY))
    drawCircle(pupil, r * 0.1f, Offset(cx + r * 0.34f, eyeY))

    // smile
    val smile = Path().apply {
        moveTo(cx - r * 0.35f, cy + r * 0.25f)
        quadraticBezierTo(cx, cy + r * 0.55f, cx + r * 0.35f, cy + r * 0.25f)
    }
    drawPath(smile, Arcade.Ink, style = Stroke(width = 3f, cap = StrokeCap.Round))

    // brain glow when injected
    if (loaded) {
        drawCircle(
            color = (if (flash) Arcade.NeonCyan else Arcade.Fluid).copy(alpha = 0.25f),
            radius = r * 1.15f,
            center = Offset(cx, cy - r * 0.15f),
        )
        // little "★" spark
        drawCircle(Arcade.NeonYellow, r * 0.12f, Offset(cx + r * 0.85f, cy - r * 0.9f))
    }
}
