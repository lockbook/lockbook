package app.lockbook.model

import android.content.Context
import android.text.Editable
import android.text.Spanned
import android.text.style.ForegroundColorSpan
import android.text.style.StrikethroughSpan
import androidx.core.content.res.ResourcesCompat
import app.lockbook.R
import io.noties.markwon.Markwon
import io.noties.markwon.core.MarkwonTheme
import io.noties.markwon.core.spans.BlockQuoteSpan
import io.noties.markwon.core.spans.CodeBlockSpan
import io.noties.markwon.core.spans.HeadingSpan
import io.noties.markwon.editor.*
import io.noties.markwon.editor.handler.EmphasisEditHandler
import io.noties.markwon.editor.handler.StrongEmphasisEditHandler

class CustomPunctuationSpan internal constructor(color: Int) : ForegroundColorSpan(color)

object MarkdownModel {
    fun createMarkdownEditor(context: Context): MarkwonEditor {
        val theme = MarkwonTheme.builderWithDefaults(context).build()

        return MarkwonEditor.builder(Markwon.create(context))
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
            .useEditHandler(CodeEditHandler(theme))
            .useEditHandler(CodeBlockEditHandler(theme))
            .useEditHandler(BlockQuoteEditHandler(theme))
            .useEditHandler(HeadingEditHandler(theme))
            .useEditHandler(StrikethroughEditHandler())
            .build()
    }
}

class CodeEditHandler(private val theme: MarkwonTheme) : AbstractEditHandler<CodeEditHandler>() {
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
            MarkwonEditorUtils.findDelimited(input, spanStart, "`")
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

class CodeBlockEditHandler(private val theme: MarkwonTheme) : AbstractEditHandler<CodeBlockSpan>() {
    override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
        builder.persistSpan(
            CodeBlockSpan::class.java
        ) { CodeBlockSpan(theme) }
    }

    override fun handleMarkdownSpan(
        persistedSpans: PersistedSpans,
        editable: Editable,
        input: String,
        span: CodeBlockSpan,
        spanStart: Int,
        spanTextLength: Int
    ) {
        val match =
            MarkwonEditorUtils.findDelimited(input, spanStart, "```", "```")
        if (match != null) {
            editable.setSpan(
                persistedSpans[CodeBlockSpan::class.java],
                match.start(),
                match.end(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    override fun markdownSpanType(): Class<CodeBlockSpan> = CodeBlockSpan::class.java
}

class BlockQuoteEditHandler(private val theme: MarkwonTheme) : AbstractEditHandler<BlockQuoteSpan>() {
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
        editable.setSpan(
            persistedSpans.get(BlockQuoteSpan::class.java),
            spanStart,
            spanStart + spanTextLength,
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
        )
    }

    override fun markdownSpanType(): Class<BlockQuoteSpan> = BlockQuoteSpan::class.java
}

class HeadingEditHandler(private val theme: MarkwonTheme) : AbstractEditHandler<HeadingSpan>() {
    override fun init(markwon: Markwon) {}

    override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
        builder
            .persistSpan(
                Head1::class.java
            ) { Head1(theme) }
            .persistSpan(
                Head2::class.java
            ) { Head2(theme) }
    }

    override fun handleMarkdownSpan(
        persistedSpans: PersistedSpans,
        editable: Editable,
        input: String,
        span: HeadingSpan,
        spanStart: Int,
        spanTextLength: Int
    ) {

        val type = when (span.level) {
            1 -> Head1::class.java
            2 -> Head2::class.java
            else -> null
        }
        if (type != null) {
            val index = input.indexOf('\n', spanStart + spanTextLength)

            val end = if (index < 0) input.length else index

            editable.setSpan(
                persistedSpans[type],
                spanStart,
                end,
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    override fun markdownSpanType(): Class<HeadingSpan> {
        return HeadingSpan::class.java
    }

    private class Head1(theme: MarkwonTheme) : HeadingSpan(theme, 1)
    private class Head2(theme: MarkwonTheme) : HeadingSpan(theme, 2)
}

class StrikethroughEditHandler : AbstractEditHandler<StrikethroughSpan>() {
    override fun configurePersistedSpans(builder: PersistedSpans.Builder) {
        builder.persistSpan(
            StrikethroughSpan::class.java
        ) { StrikethroughSpan() }
    }

    override fun handleMarkdownSpan(
        persistedSpans: PersistedSpans,
        editable: Editable,
        input: String,
        span: StrikethroughSpan,
        spanStart: Int,
        spanTextLength: Int
    ) {
        val match =
            MarkwonEditorUtils.findDelimited(input, spanStart, "~~")
        if (match != null) {
            editable.setSpan(
                persistedSpans[StrikethroughSpan::class.java],
                match.start(),
                match.end(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE
            )
        }
    }

    override fun markdownSpanType(): Class<StrikethroughSpan> = StrikethroughSpan::class.java
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