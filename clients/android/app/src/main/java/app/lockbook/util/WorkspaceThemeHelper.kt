package app.lockbook.util

import android.content.Context
import android.graphics.Color
import app.lockbook.R
import app.lockbook.workspace.Workspace
import com.google.android.material.color.MaterialColors
import kotlin.math.abs
import kotlin.math.min

data class WorkspaceTheme(
    val isDark: Boolean,
    val dim: WorkspaceThemeVariant,
    val lightPrefs: WorkspaceThemePreferences,
    val bright: WorkspaceThemeVariant,
    val darkPrefs: WorkspaceThemePreferences,
)

data class WorkspaceThemeVariant(
    val black: Int,
    val grey: Int,
    val red: Int,
    val green: Int,
    val yellow: Int,
    val blue: Int,
    val magenta: Int,
    val cyan: Int,
    val white: Int,
)

data class WorkspaceThemePreferences(
    val primary: String,
    val secondary: String,
    val tertiary: String,
    val quaternary: String,
)

object WorkspaceThemeHelper {
    private const val PALETTE_RED = "red"
    private const val PALETTE_GREEN = "green"
    private const val PALETTE_YELLOW = "yellow"
    private const val PALETTE_BLUE = "blue"
    private const val PALETTE_MAGENTA = "magenta"
    private const val PALETTE_CYAN = "cyan"

    fun materialTheme(
        context: Context,
        darkMode: Boolean,
    ): WorkspaceTheme {
        val defaultTheme = Workspace.defaultTheme(darkMode) as WorkspaceTheme
        val materialPrimary =
            context.getMaterialColorOrFallback(com.google.android.material.R.attr.colorPrimaryFixed, R.color.md_theme_primary)
        val materialSecondary =
            context.getMaterialColorOrFallback(com.google.android.material.R.attr.colorSecondary, R.color.md_theme_secondary)
        val materialTertiary =
            context.getMaterialColorOrFallback(com.google.android.material.R.attr.colorTertiary, R.color.md_theme_tertiary)
        val materialSurface =
            context.getMaterialColorOrFallback(com.google.android.material.R.attr.colorSurface, R.color.md_theme_surface)
        val materialSurfaceVariant =
            context.getMaterialColorOrFallback(
                com.google.android.material.R.attr.colorSurfaceVariant,
                R.color.md_theme_surfaceVariant,
            )
        val materialOnSurface =
            context.getMaterialColorOrFallback(com.google.android.material.R.attr.colorOnSurface, R.color.md_theme_onSurface)
        val materialOnSurfaceVariant =
            context.getMaterialColorOrFallback(
                com.google.android.material.R.attr.colorOnSurfaceVariant,
                R.color.md_theme_onSurfaceVariant,
            )

        val harmonizedDim =
            defaultTheme.dim.withAccentColors(
                defaultTheme.dim.accentColors().map { MaterialColors.harmonize(it, materialPrimary) },
            )
        val harmonizedBright =
            defaultTheme.bright.withAccentColors(
                defaultTheme.bright.accentColors().map {
                    MaterialColors.harmonize(it, materialPrimary)
                },
            )

        val renderedAccentSlots =
            if (darkMode) {
                harmonizedBright.accentColors()
            } else {
                harmonizedDim.accentColors()
            }
        val prefs =
            pickWorkspacePreferences(
                intArrayOf(materialPrimary, materialSecondary, materialTertiary),
                renderedAccentSlots,
            )
        val materialRoleColors =
            mapOf(
                prefs.primary to materialPrimary,
                prefs.secondary to materialSecondary,
                prefs.tertiary to materialTertiary,
            )

        val dim =
            harmonizedDim
                .copy(
                    black = if (darkMode) materialSurface else materialOnSurface,
                    grey = materialOnSurfaceVariant,
                    white = if (darkMode) materialOnSurface else materialSurface,
                ).withPaletteColors(materialRoleColors)

        val bright =
            harmonizedBright
                .copy(
                    black = materialOnSurface,
                    grey = if (darkMode) materialOnSurfaceVariant else materialSurfaceVariant,
                    white = materialSurface,
                ).withPaletteColors(materialRoleColors)

        return WorkspaceTheme(
            isDark = darkMode,
            dim = dim,
            lightPrefs = prefs,
            bright = bright,
            darkPrefs = prefs,
        )
    }

    private fun WorkspaceThemeVariant.accentColors(): List<Int> = listOf(red, green, yellow, blue, magenta, cyan)

    private fun WorkspaceThemeVariant.withAccentColors(colors: List<Int>): WorkspaceThemeVariant =
        copy(
            red = colors[0],
            green = colors[1],
            yellow = colors[2],
            blue = colors[3],
            magenta = colors[4],
            cyan = colors[5],
        )

    private fun WorkspaceThemeVariant.withPaletteColors(colors: Map<String, Int>): WorkspaceThemeVariant =
        colors.entries.fold(this) { variant, (palette, color) ->
            variant.withPaletteColor(palette, color)
        }

    private fun WorkspaceThemeVariant.withPaletteColor(
        palette: String,
        color: Int,
    ): WorkspaceThemeVariant =
        when (palette) {
            PALETTE_RED -> copy(red = color)
            PALETTE_GREEN -> copy(green = color)
            PALETTE_YELLOW -> copy(yellow = color)
            PALETTE_BLUE -> copy(blue = color)
            PALETTE_MAGENTA -> copy(magenta = color)
            PALETTE_CYAN -> copy(cyan = color)
            else -> this
        }

    private fun pickWorkspacePreferences(
        materialRoles: IntArray,
        renderedAccentSlots: List<Int>,
    ): WorkspaceThemePreferences {
        val paletteSlots =
            mutableListOf(
                PALETTE_RED to renderedAccentSlots[0],
                PALETTE_GREEN to renderedAccentSlots[1],
                PALETTE_YELLOW to renderedAccentSlots[2],
                PALETTE_BLUE to renderedAccentSlots[3],
                PALETTE_MAGENTA to renderedAccentSlots[4],
                PALETTE_CYAN to renderedAccentSlots[5],
            )

        val picked =
            materialRoles
                .map { roleColor ->
                    val best =
                        paletteSlots.minByOrNull { (_, slotColor) ->
                            colorMatchScore(roleColor, slotColor)
                        } ?: paletteSlots.first()
                    paletteSlots.remove(best)
                    best.first
                }.toMutableList()

        picked += paletteSlots.maxByOrNull { (_, slotColor) -> saturation(slotColor) }?.first ?: PALETTE_CYAN
        return WorkspaceThemePreferences(
            primary = picked[0],
            secondary = picked[1],
            tertiary = picked[2],
            quaternary = picked[3],
        )
    }

    private fun colorMatchScore(
        materialColor: Int,
        paletteColor: Int,
    ): Float {
        val materialHsv = FloatArray(3)
        val paletteHsv = FloatArray(3)
        Color.colorToHSV(materialColor, materialHsv)
        Color.colorToHSV(paletteColor, paletteHsv)
        return hueDistance(materialHsv[0], paletteHsv[0]) + (abs(materialHsv[1] - paletteHsv[1]) * 30f)
    }

    private fun hueDistance(
        a: Float,
        b: Float,
    ): Float {
        val diff = abs(a - b)
        return min(diff, 360f - diff)
    }

    private fun saturation(color: Int): Float {
        val hsv = FloatArray(3)
        Color.colorToHSV(color, hsv)
        return hsv[1]
    }
}
