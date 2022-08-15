package app.lockbook.model

import android.content.Context
import android.text.Editable
import android.text.Spanned
import android.text.style.ForegroundColorSpan
import androidx.core.content.res.ResourcesCompat
import app.lockbook.R
import io.noties.markwon.Markwon
import io.noties.markwon.core.MarkwonTheme
import io.noties.markwon.core.spans.BlockQuoteSpan
import io.noties.markwon.editor.EditHandler
import io.noties.markwon.editor.MarkwonEditor
import io.noties.markwon.editor.MarkwonEditorUtils
import io.noties.markwon.editor.PersistedSpans
import io.noties.markwon.editor.handler.EmphasisEditHandler
import io.noties.markwon.editor.handler.StrongEmphasisEditHandler

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)

object MarkdownModel {
    fun createMarkdownEditor(context: Context, theme: MarkwonTheme): MarkwonEditor =
        MarkwonEditor.builder(Markwon.create(context))
            .punctuationSpan(
                CustomPunctuationSpan::class.java
            ) {
                CustomPunctuationSpan(
                    ResourcesCompat.getColor(
                        context.resources,
                        R.color.md_theme_primary,
                        null
                    )
                )
            }
            .useEditHandler(EmphasisEditHandler())
            .useEditHandler(StrongEmphasisEditHandler())
            .build()

}

class BlockQuoteEditHandler(val theme: MarkwonTheme) : EditHandler<BlockQuoteSpan> {
    override fun init(markwon: Markwon) {}

    override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
        builder.persistSpan(
            BlockQuoteSpan::class.java
        ) { BlockQuoteSpan(theme) }
    }

    override fun handleMarkdownSpan(
        persistedSpans: PersistedSpans,
        editable: Editable,
        input: String,
        span: BlockQuoteSpan,
        spanStart: Int,
        spanTextLength: Int
    ) {
        val match =
            MarkwonEditorUtils.findDelimited(input, spanStart, ">", "\n")
        if (match != null) {
            editable.setSpan(
                persistedSpans[BlockQuoteSpan::class.java],
                match.start(),
                match.end(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    override fun markdownSpanType(): Class<BlockQuoteSpan> = BlockQuoteSpan::class.java
}

class CodeEditHandler(val theme: MarkwonTheme) : EditHandler<CodeEditHandler> {
    override fun init(markwon: Markwon) {}

    override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
        builder.persistSpan(
            CodeEditHandler::class.java
        ) { CodeEditHandler(theme) }
    }

    override fun handleMarkdownSpan(
        persistedSpans: PersistedSpans,
        editable: Editable,
        input: String,
        span: CodeEditHandler,
        spanStart: Int,
        spanTextLength: Int
    ) {
        val match =
            MarkwonEditorUtils.findDelimited(input, spanStart, "`", "`")
        if (match != null) {
            editable.setSpan(
                persistedSpans[CodeEditHandler::class.java],
                match.start(),
                match.end(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    override fun markdownSpanType(): Class<CodeEditHandler> = CodeEditHandler::class.java
}

class CodeEditHandler(val theme: MarkwonTheme) : EditHandler<CodeEditHandler> {
    override fun init(markwon: Markwon) {}

    override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
        builder.persistSpan(
            CodeEditHandler::class.java
        ) { CodeEditHandler(theme) }
    }

    override fun handleMarkdownSpan(
        persistedSpans: PersistedSpans,
        editable: Editable,
        input: String,
        span: CodeEditHandler,
        spanStart: Int,
        spanTextLength: Int
    ) {
        val match =
            MarkwonEditorUtils.findDelimited(input, spanStart, "`", "`")
        if (match != null) {
            editable.setSpan(
                persistedSpans[CodeEditHandler::class.java],
                match.start(),
                match.end(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    override fun markdownSpanType(): Class<CodeEditHandler> = CodeEditHandler::class.java
}


//            .useEditHandler(object : AbstractEditHandler<StrongEmphasisSpan>() {
//                override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
//                    builder.persistSpan(
//                        StrongEmphasisSpan::class.java
//                    ) { StrongEmphasisSpan() }
//                }
//
//                override fun handleMarkdownSpan(
//                    persistedSpans: PersistedSpans,
//                    editable: Editable,
//                    input: String,
//                    span: StrongEmphasisSpan,
//                    spanStart: Int,
//                    spanTextLength: Int
//                ) {
//                    val match =
//                        MarkwonEditorUtils.findDelimited(input, spanStart, "**", "**")
//                    if (match != null) {
//                        editable.setSpan(
//                            persistedSpans[StrongEmphasisSpan::class.java],
//                            match.start(),
//                            match.end(),
//                            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
//                        )
//                    }
//                }
//
//                override fun markdownSpanType(): Class<StrongEmphasisSpan> = StrongEmphasisSpan::class.java
//
//            })
//            .useEditHandler(object : AbstractEditHandler<EmphasisSpan>() {
//                override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
//                    builder.persistSpan(
//                        EmphasisSpan::class.java
//                    ) { EmphasisSpan() }
//                }
//
//                override fun handleMarkdownSpan(
//                    persistedSpans: PersistedSpans,
//                    editable: Editable,
//                    input: String,
//                    span: EmphasisSpan,
//                    spanStart: Int,
//                    spanTextLength: Int
//                ) {
//                    val match =
//                        MarkwonEditorUtils.findDelimited(input, spanStart, "__", "__")
//                    if (match != null) {
//                        editable.setSpan(
//                            persistedSpans[EmphasisSpan::class.java],
//                            match.start(),
//                            match.end(),
//                            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
//                        )
//                    }
//                }
//
//                override fun markdownSpanType(): Class<EmphasisSpan> = EmphasisSpan::class.java
//            })
//            .useEditHandler(object : AbstractEditHandler<CodeSpan>() {
//                override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
//                    builder.persistSpan(
//                        CodeSpan::class.java
//                    ) { CodeSpan(theme) }
//                }
//
//                override fun handleMarkdownSpan(
//                    persistedSpans: PersistedSpans,
//                    editable: Editable,
//                    input: String,
//                    span: CodeSpan,
//                    spanStart: Int,
//                    spanTextLength: Int
//                ) {
//                    val match =
//                        MarkwonEditorUtils.findDelimited(input, spanStart, "`", "`")
//                    if (match != null) {
//                        editable.setSpan(
//                            persistedSpans[CodeSpan::class.java],
//                            match.start(),
//                            match.end(),
//                            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
//                        )
//                    }
//                }
//
//                override fun markdownSpanType(): Class<CodeSpan> = CodeSpan::class.java
//            })