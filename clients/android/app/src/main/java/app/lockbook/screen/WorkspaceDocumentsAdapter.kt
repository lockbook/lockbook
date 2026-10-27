package app.lockbook.screen

import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.BackgroundColorSpan
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import androidx.core.graphics.ColorUtils
import androidx.core.view.isVisible
import androidx.lifecycle.LifecycleCoroutineScope
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.WorkspaceDocumentCardBinding
import app.lockbook.model.WorkspaceDocumentItem
import app.lockbook.util.DocumentPreview
import app.lockbook.util.DocumentPreviewLoader
import app.lockbook.util.harmonizedSeedColors
import app.lockbook.util.identityColorBucket
import app.lockbook.workspace.MarkdownExcerpt
import com.google.android.material.color.ColorRoles
import io.noties.markwon.AbstractMarkwonPlugin
import io.noties.markwon.Markwon
import io.noties.markwon.core.MarkwonTheme
import io.noties.markwon.ext.tasklist.TaskListPlugin
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File

internal class WorkspaceDocumentsAdapter(
    private val scope: LifecycleCoroutineScope,
    private val previews: DocumentPreviewLoader,
    private val openDocument: (File) -> Unit,
    private val showCoverMenu: (File, View) -> Unit,
) : ListAdapter<WorkspaceDocumentItem, RecyclerView.ViewHolder>(DocumentDiff()) {
    fun refreshCover(id: String) {
        previews.invalidate(id)
        val position = currentList.indexOfFirst { it.file.id == id }
        if (position >= 0) notifyItemChanged(position)
    }

    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ): RecyclerView.ViewHolder {
        val inflater = LayoutInflater.from(parent.context)
        return CardHolder(WorkspaceDocumentCardBinding.inflate(inflater, parent, false))
    }

    override fun onBindViewHolder(
        holder: RecyclerView.ViewHolder,
        position: Int,
    ) {
        (holder as CardHolder).bind(getItem(position))
    }

    override fun onViewRecycled(holder: RecyclerView.ViewHolder) {
        (holder as CardHolder).imageJob?.cancel()
        super.onViewRecycled(holder)
    }

    private inner class CardHolder(
        private val binding: WorkspaceDocumentCardBinding,
    ) : RecyclerView.ViewHolder(binding.root) {
        var imageJob: Job? = null
        private var previewFocus = 0

        init {
            binding.root.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
                binding.root.post { updateCoverHeight() }
                binding.markdownPreview.post { positionPreview() }
            }
        }

        private fun positionPreview() {
            val view = binding.markdownPreview
            val layout = view.layout ?: return
            if (!view.isVisible) return
            val available = (view.height - view.totalPaddingTop - view.totalPaddingBottom).coerceAtLeast(1)
            val line = layout.getLineForOffset(previewFocus.coerceIn(0, view.text.length))
            val top = (layout.getLineTop(line) - available / 3).coerceAtLeast(0)
            view.scrollTo(0, top.coerceAtMost((layout.height - available).coerceAtLeast(0)))
        }

        private fun updateCoverHeight() {
            val position = bindingAdapterPosition
            if (position == RecyclerView.NO_POSITION) return
            // Repeat short, medium, and tall previews by position in the grid.
            val baseHeight = binding.root.resources.getDimensionPixelSize(R.dimen.workspace_card_preview_base_height)
            val height = baseHeight * (position % 3 + 1) / 3
            if (binding.coverArea.layoutParams.height != height) {
                binding.coverArea.layoutParams = binding.coverArea.layoutParams.apply { this.height = height }
            }
        }

        private fun applyCardColors(colors: ColorRoles): Int {
            val coverColor = ColorUtils.blendARGB(colors.accentContainer, Color.BLACK, 0.01f)
            binding.root.strokeColor = ColorUtils.blendARGB(binding.root.cardBackgroundColor.defaultColor, colors.accent, 0.18f)
            binding.cover.setBackgroundColor(coverColor)
            binding.markdownPreview.setTextColor(colors.onAccentContainer)
            binding.previewFade.background =
                GradientDrawable(
                    GradientDrawable.Orientation.TOP_BOTTOM,
                    intArrayOf(ColorUtils.setAlphaComponent(coverColor, 0), coverColor),
                )
            binding.title.setTextColor(colors.accent)
            binding.coverMenu.iconTint = ColorStateList.valueOf(colors.accent)
            return coverColor
        }

        fun bind(item: WorkspaceDocumentItem) {
            imageJob?.cancel()
            val context = itemView.context
            val file = item.file
            val hue = identityColorBucket(file.id) * 45f
            val colors = harmonizedSeedColors(context, Color.HSVToColor(floatArrayOf(hue, 0.475f, 0.7f)))
            val coverColor = applyCardColors(colors)
            binding.cover.setImageDrawable(null)
            binding.cover.setPadding(0, 0, 0, 0)
            binding.markdownPreview.isVisible = false
            binding.markdownPreview.scrollTo(0, 0)
            binding.previewFade.isVisible = false
            binding.root.post { updateCoverHeight() }
            binding.title.text = item.name
            binding.coverMenu.contentDescription = context.getString(R.string.workspace_cover_options, item.name)
            binding.coverMenu.setOnClickListener { showCoverMenu(file, binding.coverMenu) }
            binding.root.setOnClickListener { openDocument(file) }
            imageJob =
                scope.launch {
                    val preview =
                        try {
                            previews.load(file.id, item.name, item.modified, item.cursor)
                        } catch (cancelled: CancellationException) {
                            throw cancelled
                        } catch (_: Exception) {
                            null
                        }
                    when (preview) {
                        is DocumentPreview.Image -> {
                            preview.seedColor?.let { applyCardColors(harmonizedSeedColors(context, it)) }
                            binding.cover.scaleType = ImageView.ScaleType.CENTER_CROP
                            binding.cover.setPadding(0, 0, 0, 0)
                            binding.cover.setImageBitmap(preview.bitmap)
                        }

                        is DocumentPreview.Markdown -> {
                            showMarkdown(preview.excerpt, colors.accent, coverColor)
                        }

                        null -> {
                            Unit
                        }
                    }
                }
        }

        private suspend fun showMarkdown(
            excerpt: MarkdownExcerpt,
            accent: Int,
            background: Int,
        ) {
            val context = itemView.context
            val rendered =
                withContext(Dispatchers.Default) {
                    val renderer =
                        Markwon
                            .builder(context)
                            .usePlugin(TaskListPlugin.create(accent, accent, background))
                            .usePlugin(
                                object : AbstractMarkwonPlugin() {
                                    override fun configureTheme(builder: MarkwonTheme.Builder) {
                                        builder
                                            .headingTextSizeMultipliers(floatArrayOf(1.25f, 1.18f, 1.1f, 1f, 1f, 1f))
                                            .headingBreakHeight(0)
                                            .linkColor(accent)
                                    }
                                },
                            ).build()
                    val result = SpannableStringBuilder(renderer.toMarkdown(excerpt.markdown))
                    val focus = renderer.toMarkdown(excerpt.focusLine).toString().trim()
                    val start = if (focus.isNotBlank()) result.indexOf(focus) else -1
                    if (start >= 0) {
                        result.setSpan(
                            BackgroundColorSpan(ColorUtils.blendARGB(background, accent, 0.12f)),
                            start,
                            start + focus.length,
                            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE,
                        )
                    }
                    result to start
                }
            binding.markdownPreview.text =
                if (rendered.first.isBlank()) context.getString(R.string.workspace_blank_note_preview) else rendered.first
            previewFocus = rendered.second.coerceAtLeast(0)
            binding.markdownPreview.isVisible = true
            binding.previewFade.isVisible = true
            binding.markdownPreview.post { positionPreview() }
        }
    }

    private class DocumentDiff : DiffUtil.ItemCallback<WorkspaceDocumentItem>() {
        override fun areItemsTheSame(
            oldItem: WorkspaceDocumentItem,
            newItem: WorkspaceDocumentItem,
        ): Boolean = oldItem.file.id == newItem.file.id

        override fun areContentsTheSame(
            oldItem: WorkspaceDocumentItem,
            newItem: WorkspaceDocumentItem,
        ): Boolean = oldItem.name == newItem.name && oldItem.modified == newItem.modified && oldItem.cursor == newItem.cursor
    }
}
