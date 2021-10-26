package app.lockbook.util

import android.content.res.Configuration
import android.graphics.Color
import app.lockbook.model.DrawingViewModel
import com.beust.klaxon.Json
import java.util.LinkedHashMap
import kotlin.properties.Delegates

data class Drawing(
    var scale: Float = 1f,
    @Json(name = "translation_x")
    var translationX: Float = 0f,
    @Json(name = "translation_y")
    var translationY: Float = 0f,
    var strokes: MutableList<Stroke> = mutableListOf(),
    var theme: LinkedHashMap<String, ColorRGB>? = null,
) {

    @Json(ignored = true)
    lateinit var model: DrawingViewModel
    @Json(ignored = true)
    var isDirty: Boolean by Delegates.observable(false) { property, oldValue, newValue ->
        if (newValue && !oldValue) {
            model.waitAndSaveContents()
        }
    }

    fun set(drawing: Drawing) {
        scale = drawing.scale
        translationX = drawing.translationX
        translationY = drawing.translationY
        strokes = drawing.strokes
        theme = drawing.theme
    }

    fun getARGBColor(uiMode: Int, colorAlias: ColorAlias, alpha: Int = 255): Int? {
        val modifiedColorAlias = when (colorAlias) {
            ColorAlias.White -> if (uiMode == Configuration.UI_MODE_NIGHT_NO) ColorAlias.White else ColorAlias.Black
            ColorAlias.Black -> if (uiMode == Configuration.UI_MODE_NIGHT_NO) ColorAlias.Black else ColorAlias.White
            else -> colorAlias
        }

        val colorRGB = (theme ?: DEFAULT_THEME)[modifiedColorAlias.name] ?: return null
        return Color.argb(alpha, colorRGB.r, colorRGB.g, colorRGB.b)
    }

    fun themeToARGBColors(uiMode: Int): LinkedHashMap<ColorAlias, Int?> {
        return linkedMapOf(
            Pair(ColorAlias.White, getARGBColor(uiMode, ColorAlias.White)),
            Pair(ColorAlias.Black, getARGBColor(uiMode, ColorAlias.Black)),
            Pair(ColorAlias.Red, getARGBColor(uiMode, ColorAlias.Red)),
            Pair(ColorAlias.Green, getARGBColor(uiMode, ColorAlias.Green)),
            Pair(ColorAlias.Yellow, getARGBColor(uiMode, ColorAlias.Yellow)),
            Pair(ColorAlias.Blue, getARGBColor(uiMode, ColorAlias.Blue)),
            Pair(ColorAlias.Magenta, getARGBColor(uiMode, ColorAlias.Magenta)),
            Pair(ColorAlias.Cyan, getARGBColor(uiMode, ColorAlias.Cyan))
        )
    }

    fun clone(): Drawing {
        val drawing = Drawing(
            scale,
            translationX,
            translationY,
            strokes.map { stroke ->
                Stroke(
                    stroke.pointsX.toMutableList(),
                    stroke.pointsY.toMutableList(),
                    stroke.pointsGirth.toMutableList(),
                    stroke.color,
                    stroke.alpha
                )
            }.toMutableList(),
            theme
        )
        drawing.model = model
        drawing.isDirty = isDirty
        return drawing
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
    val alpha: Float
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

enum class SupportedImageFormats {
    Png,
    Jpeg,
    Pnm,
    Tga,
    Farbfeld,
    Bmp,
}

val DEFAULT_THEME = linkedMapOf(
    Pair(ColorAlias.White.name, ColorRGB(0xFF, 0xFF, 0xFF)),
    Pair(ColorAlias.Black.name, ColorRGB(0x00, 0x00, 0x00)),
    Pair(ColorAlias.Red.name, ColorRGB(0xFF, 0x00, 0x00)),
    Pair(ColorAlias.Green.name, ColorRGB(0x00, 0xFF, 0x00)),
    Pair(ColorAlias.Yellow.name, ColorRGB(0xFF, 0xFF, 0x00)),
    Pair(ColorAlias.Blue.name, ColorRGB(0x00, 0x00, 0xFF)),
    Pair(ColorAlias.Magenta.name, ColorRGB(0xFF, 0x00, 0xFF)),
    Pair(ColorAlias.Cyan.name, ColorRGB(0x00, 0xFF, 0xFF)),
)

val IMAGE_EXPORT_TYPE = SupportedImageFormats.Jpeg
