package app.lockbook.util

import android.graphics.Color
import com.beust.klaxon.Json
import com.beust.klaxon.Klaxon
import timber.log.Timber
import java.util.LinkedHashMap

data class Drawing(
        var scale: Float = 1f,
        var translationX: Float = 0f,
        var translationY: Float = 0f,
        val strokes: MutableList<Stroke> = mutableListOf(),
        val theme: LinkedHashMap<ColorAlias, ColorRGB> = linkedMapOf(
                Pair(ColorAlias.Black, ColorRGB(0x00, 0x00, 0x00)),
                Pair(ColorAlias.Red, ColorRGB(0xFF, 0x00, 0x00)),
                Pair(ColorAlias.Green, ColorRGB(0x00, 0xFF, 0x00)),
                Pair(ColorAlias.Yellow, ColorRGB(0xFF, 0xFF, 0x00)),
                Pair(ColorAlias.Blue, ColorRGB(0x00, 0x00, 0xFF)),
                Pair(ColorAlias.Magenta, ColorRGB(0xFF, 0x00, 0xFF)),
                Pair(ColorAlias.Cyan, ColorRGB(0x00, 0xFF, 0xFF)),
                Pair(ColorAlias.White, ColorRGB(0xFF, 0xFF, 0xFF))
        )
) {
    fun getColorFromAlias(colorAlias: ColorAlias, alpha: Int): Int {
        Timber.e("${Klaxon().toJsonString(theme)}, ${colorAlias.name}")
        val colorRGB = theme[colorAlias]!!
        return Color.argb(alpha, colorRGB.r, colorRGB.g, colorRGB.b)
    }
}

data class Stroke(
        @Json(name = "points_x")
        val pointsX: MutableList<Float>,
        @Json(name = "points_y")
        val pointsY: MutableList<Float>,
        @Json(name = "points_girth")
        val pointsGirth: MutableList<Float>,
        val color: ColorAlias,
        val alpha: Int
)

enum class ColorAlias {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

data class ColorRGB(
        val r: Int,
        val g: Int,
        val b: Int,
)

