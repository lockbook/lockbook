package app.lockbook.model

import app.lockbook.util.identityColorBucket
import java.util.UUID
import kotlin.random.Random

internal data class DocumentCardAppearance(
    val hue: Float,
    val coverThirds: Int,
)

/** Shared eight-color identity palette, with UUID-seeded heights in thirds of a fixed base height. */
internal fun documentCardAppearance(identity: String): DocumentCardAppearance {
    val uuid = UUID.fromString(identity)
    val random = Random(uuid.mostSignificantBits xor java.lang.Long.rotateLeft(uuid.leastSignificantBits, 17))
    // Preserve the existing height sequence now that colors no longer consume this random value.
    random.nextFloat()
    return DocumentCardAppearance(hue = identityColorBucket(identity) * 45f, coverThirds = random.nextInt(1, 4))
}

/** Predict masonry placement so a preferred height cannot flatten neighboring column bottoms. */
internal fun documentCardCoverHeights(
    identities: List<String>,
    columns: Int,
    baseHeight: Int,
    footerHeight: Int,
    gap: Int,
): List<Int> {
    require(columns > 0 && baseHeight > 0)
    val bottoms = IntArray(columns)
    return identities.mapIndexed { index, identity ->
        val preferred = documentCardAppearance(identity).coverThirds
        val column = bottoms.indices.minBy { bottoms[it] }
        val choices =
            when {
                index == 0 -> listOf(1)
                index == 1 && columns == 2 -> listOf(2 + preferred % 2)
                index < columns -> listOf(index + 1)
                else -> (0..2).map { (preferred - 1 + it) % 3 + 1 }
            }

        fun clearance(thirds: Int): Int {
            val bottom = bottoms[column] + baseHeight * thirds / 3 + footerHeight + gap
            return bottoms.indices
                .filter { it != column }
                .minOfOrNull { kotlin.math.abs(bottom - bottoms[it]) } ?: Int.MAX_VALUE
        }
        val thirds = choices.firstOrNull { clearance(it) >= gap.coerceAtLeast(1) } ?: choices.maxBy(::clearance)
        val height = baseHeight * thirds / 3
        bottoms[column] += height + footerHeight + gap
        height
    }
}
