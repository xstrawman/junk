package dev.xstrawman.junk.ui

/**
 * Long horizontal ASCII syringe = the download progress bar.
 *
 * As progress increases, magenta "JUNK" fluid is drawn up the barrel
 * from the needle toward the plunger (fills left→right).
 *
 * Example at ~50%:
 *
 *      ||
 *   [====]════════════════════════════════════════════════
 *   ║██████████████░░░░░░░░░░░░║═══════════════════◎───▶
 *   [====] PLUNGER    J U N K   fluid          NEEDLE tip
 */
object AsciiSyringe {
    fun render(progress: Float, width: Int = 28, animTick: Int = 0): String {
        val p = progress.coerceIn(0f, 1f)
        val w = width.coerceIn(10, 40)
        val filled = (p * w).toInt().coerceIn(0, w)
        val empty = w - filled
        val pct = (p * 100).toInt()

        // Body of fluid uses block chars; empty is light shade
        val fluid = "█".repeat(filled)
        val air = "░".repeat(empty)

        // Plunger "thumb rest" pulses while active
        val thumb = if (animTick % 2 == 0) "█" else "▓"
        val drop = when {
            p <= 0f -> " "
            p >= 1f -> "◆"
            animTick % 3 == 0 -> "·"
            animTick % 3 == 1 -> "•"
            else -> "●"
        }

        return buildString {
            // silkscreen label
            appendLine("   JUNK HYPO  ·  progress = barrel fill  ·  $pct%")
            // top outline of long barrel
            append("    ")
            append(thumb)
            append(thumb)
            append("╔")
            append("═".repeat(w))
            appendLine("╗")
            // main syringe body: plunger | barrel with fluid | hub | needle
            append("  ═╩╩╣")
            append(fluid)
            append(air)
            append("╠")
            append("═".repeat(4))
            append("◎")
            append("───▶")
            appendLine(" $drop")
            // bottom outline
            append("    ")
            append(thumb)
            append(thumb)
            append("╚")
            append("═".repeat(w))
            appendLine("╝")
            // legend under barrel
            append("    ^plunger  ")
            if (filled > 0) {
                val tag = "JUNK".take(filled.coerceAtMost(4))
                append(tag)
                if (filled > 4) append(" ".repeat((filled - 4).coerceAtMost(8)))
            } else {
                append("(empty)")
            }
            append(" ".repeat(4))
            appendLine("needle▶")
        }
    }

    fun renderCompact(progress: Float, width: Int = 24): String {
        val p = progress.coerceIn(0f, 1f)
        val w = width.coerceIn(8, 36)
        val filled = (p * w).toInt()
        val bar = "█".repeat(filled) + "░".repeat(w - filled)
        val pct = (p * 100).toInt().toString().padStart(3)
        return "[=|$bar|◎──▶] $pct%"
    }
}
