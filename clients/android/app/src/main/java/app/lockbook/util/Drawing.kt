package app.lockbook.util

import com.beust.klaxon.Json
import java.util.LinkedHashMap

data class Drawing(
        var scale: Float = 1f,
        var translationX: Float = 0f,
        var translationY: Float = 0f,
        val strokes: MutableList<Stroke> = mutableListOf(),
        val theme: LinkedHashMap<ColorAlias, ColorRGB> = linkedMapOf()
)

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