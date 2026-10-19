package app.lockbook.model

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DocumentCardAppearanceTest {
    @Test
    fun `UUID-seeded appearance stays stable across binds and UUID casing`() {
        val uuid = "b38c873e-c135-4cd0-a901-83a4c2160f42"
        assertEquals(documentCardAppearance(uuid), documentCardAppearance(uuid))
        assertEquals(documentCardAppearance(uuid), documentCardAppearance(uuid.uppercase()))
    }

    @Test
    fun `both halves of the document UUID influence the color`() {
        val original = documentCardAppearance("b38c873e-c135-4cd0-a901-83a4c2160f42")
        assertNotEquals(original.hue, documentCardAppearance("a38c873e-c135-4cd0-a901-83a4c2160f42").hue)
        assertNotEquals(original.hue, documentCardAppearance("b38c873e-c135-4cd0-a901-83a4c2160f43").hue)
    }

    @Test
    fun `first two previews start short and tall and later columns avoid matching bottoms`() {
        val identities = (0..99).map { java.util.UUID(0, it.toLong()).toString() }
        for (baseHeight in listOf(240, 480, 720)) {
            val heights = documentCardCoverHeights(identities, columns = 2, baseHeight, footerHeight = 56, gap = 12)
            assertEquals(baseHeight / 3, heights[0])
            assertTrue(heights[1] in listOf(baseHeight * 2 / 3, baseHeight))
            val bottoms = IntArray(2)
            heights.forEach { height ->
                val column = if (bottoms[0] <= bottoms[1]) 0 else 1
                bottoms[column] += height + 56 + 12
                assertTrue(kotlin.math.abs(bottoms[0] - bottoms[1]) >= 12)
            }
            assertEquals(heights, documentCardCoverHeights(identities, 2, baseHeight, 56, 12))
        }
    }

    @Test
    fun `cover heights follow thirds and UUID hashes use exactly eight color buckets`() {
        val appearances = (0..99).map { documentCardAppearance(java.util.UUID(0, it.toLong()).toString()) }
        assertEquals(setOf(1, 2, 3), appearances.map { it.coverThirds }.toSet())
        assertTrue(appearances.all { it.hue >= 0 && it.hue < 360 })
        assertEquals((0..7).map { it * 45f }.toSet(), appearances.map { it.hue }.toSet())
    }
}
