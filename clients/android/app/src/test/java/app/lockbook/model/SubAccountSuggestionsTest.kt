package app.lockbook.model

import org.junit.Assert.assertEquals
import org.junit.Test

class SubAccountSuggestionsTest {
    private val subAccounts = listOf("jane+site", "jane+agent", "jane+ci")

    private fun suggest(
        query: String,
        shared: List<String> = emptyList(),
    ) = subAccountSuggestions(query, subAccounts, "jane", shared)

    @Test
    fun `lists every sub-account before anything is typed`() {
        assertEquals(listOf("jane+ci", "jane+site", "jane+agent"), suggest(""))
    }

    @Test
    fun `leaves out sub-accounts the file is already shared with`() {
        assertEquals(listOf("jane+ci", "jane+agent"), suggest("", shared = listOf("Jane+Site", "bob")))
    }

    @Test
    fun `matches the full name, the label and the separator with the label`() {
        assertEquals(listOf("jane+site"), suggest("jane+s"))
        assertEquals(listOf("jane+site"), suggest("Si"))
        assertEquals(listOf("jane+agent"), suggest("+a"))
        assertEquals(listOf("jane+ci", "jane+site", "jane+agent"), suggest("jan"))
    }

    @Test
    fun `leaves out an exact match and other people`() {
        assertEquals(emptyList<String>(), suggest("jane+site"))
        assertEquals(emptyList<String>(), suggest("bob"))
    }

    @Test
    fun `shows at most five`() {
        val many = (1..8).map { "jane+bot$it" }
        assertEquals(many.take(5), subAccountSuggestions("", many, "jane", emptyList()))
    }
}
