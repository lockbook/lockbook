package app.lockbook.util

import android.content.Context
import android.graphics.Color
import com.google.android.material.color.ColorRoles
import com.google.android.material.color.MaterialColors
import java.util.Locale

internal fun identityColorBucket(identity: String): Int = Math.floorMod(identity.lowercase(Locale.ROOT).hashCode(), 8)

/** Shared palette for usernames and document identities, harmonized with the active Material theme. */
internal fun harmonizedIdentityColors(
    context: Context,
    identity: String,
): ColorRoles {
    val colorIndex = identityColorBucket(identity)
    val seedColor = Color.HSVToColor(floatArrayOf(colorIndex * 45f, 0.65f, 0.8f))
    return harmonizedSeedColors(context, seedColor)
}

internal fun harmonizedSeedColors(
    context: Context,
    seedColor: Int,
): ColorRoles {
    val primary = MaterialColors.getColor(context, androidx.appcompat.R.attr.colorPrimary, Color.BLUE)
    return MaterialColors.getColorRoles(context, MaterialColors.harmonize(seedColor, primary))
}
