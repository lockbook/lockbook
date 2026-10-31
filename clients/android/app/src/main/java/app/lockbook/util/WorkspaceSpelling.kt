package app.lockbook.util

import android.content.Context
import android.view.textservice.SentenceSuggestionsInfo
import android.view.textservice.SpellCheckerSession
import android.view.textservice.SuggestionsInfo
import android.view.textservice.TextInfo
import android.view.textservice.TextServicesManager
import app.lockbook.workspace.Workspace

/**
 * The system spell checker, in the keyboard's language: it answers the words
 * the editor asks about, and keeps each typo's suggestions for the edit menu.
 * Answers arrive later on the main thread, in the order they were asked for.
 */
class WorkspaceSpelling(
    context: Context,
    private val answered: () -> Unit,
) : SpellCheckerSession.SpellCheckerSessionListener {
    private val session: SpellCheckerSession? =
        (context.getSystemService(Context.TEXT_SERVICES_MANAGER_SERVICE) as? TextServicesManager)
            ?.newSpellCheckerSession(null, null, this, true)

    private val checks = ArrayDeque<List<String>>()
    private val suggestions = HashMap<String, List<String>>()

    /** Sends the words the editor asked about to the checker. */
    fun check(wgpuObj: Long) {
        val words = Workspace.spellingRequests(wgpuObj)?.split('\n') ?: return
        val session = session ?: return
        checks.addLast(words)
        session.getSentenceSuggestions(words.map { TextInfo(it) }.toTypedArray(), MAX_GUESSES)
    }

    /** The fixes the checker offered when it found `word` misspelled. */
    fun guesses(word: String): List<String> = suggestions[word].orEmpty()

    override fun onGetSentenceSuggestions(results: Array<out SentenceSuggestionsInfo>?) {
        val words = checks.removeFirstOrNull() ?: return
        if (WorkspaceView.wgpuObj == Long.MAX_VALUE) return
        // a word the checker gave no result for stays unknown
        words.forEachIndexed { i, word ->
            val result = results?.getOrNull(i) ?: return@forEachIndexed
            val typo = infos(result).firstOrNull(::looksLikeTypo)
            if (typo == null) {
                suggestions.remove(word)
            } else {
                suggestions[word] = (0 until typo.suggestionsCount).map(typo::getSuggestionAt).take(MAX_GUESSES)
            }
            Workspace.spellingAnswer(WorkspaceView.wgpuObj, word, typo == null)
        }
        answered()
    }

    override fun onGetSuggestions(results: Array<out SuggestionsInfo>?) {}

    private fun infos(sentence: SentenceSuggestionsInfo): List<SuggestionsInfo> =
        (0 until sentence.suggestionsCount).map {
            sentence.getSuggestionsInfoAt(it)
        }

    private fun looksLikeTypo(info: SuggestionsInfo): Boolean =
        info.suggestionsAttributes and SuggestionsInfo.RESULT_ATTR_LOOKS_LIKE_TYPO != 0

    companion object {
        const val MAX_GUESSES = 5
    }
}
