package app.lockbook.ui

import android.graphics.Color
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.core.os.bundleOf
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.isVisible
import androidx.core.view.updatePadding
import androidx.core.widget.addTextChangedListener
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.GridLayoutManager
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.SheetPinEmojiBinding
import app.lockbook.model.FileTreeViewModel
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.bottomsheet.BottomSheetDialogFragment
import com.google.android.material.color.MaterialColors
import com.google.android.material.textview.MaterialTextView
import com.vanniktech.emoji.Emoji
import com.vanniktech.emoji.EmojiTheming
import com.vanniktech.emoji.recent.RecentEmojiManager
import com.vanniktech.emoji.search.NoSearchEmoji
import com.vanniktech.emoji.search.SearchEmojiManager
import com.vanniktech.emoji.search.SearchEmojiResult
import com.vanniktech.emoji.variant.VariantEmojiManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class PinEmojiBottomSheetFragment : BottomSheetDialogFragment() {
    private var viewBinding: SheetPinEmojiBinding? = null
    private val binding get() = viewBinding!!
    private val model: FileTreeViewModel by activityViewModels()
    private val search = SearchEmojiManager()
    private var searchJob: Job? = null
    private var pickerReady = false
    private lateinit var recentEmoji: RecentEmojiManager
    private lateinit var variantEmoji: VariantEmojiManager
    private val resultsAdapter = PinEmojiSearchAdapter(::selectSearchResult)

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        viewBinding = SheetPinEmojiBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)
        ViewCompat.setAccessibilityHeading(binding.emojiTitle, true)
        val bottomPadding = view.paddingBottom
        ViewCompat.setOnApplyWindowInsetsListener(view) { sheet, insets ->
            val keyboard = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val navigation = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
            sheet.updatePadding(bottom = bottomPadding + (keyboard - navigation).coerceAtLeast(0))
            insets
        }
        binding.fileName.text = requireArguments().getString(FILE_NAME)
        val cellWidth = (56 * resources.displayMetrics.density).toInt()
        val grid = GridLayoutManager(requireContext(), 6)
        binding.emojiResults.layoutManager = grid
        binding.emojiResults.addOnLayoutChangeListener { _, left, _, right, _, oldLeft, _, oldRight, _ ->
            if (right - left != oldRight - oldLeft) {
                grid.spanCount = ((right - left) / cellWidth).coerceAtLeast(1)
            }
        }
        binding.emojiResults.adapter = resultsAdapter
        binding.emojiResults.itemAnimator = null
        binding.emojiSearch.addTextChangedListener { updateSearch(it?.toString().orEmpty()) }
        viewLifecycleOwner.lifecycleScope.launch {
            EmojiPickerInitializer.ensureInitialized()
            setUpPicker(view)
            pickerReady = true
            binding.emojiLoading.isVisible = false
            binding.emojiSearch.isEnabled = true
            val query =
                binding.emojiSearch.text
                    ?.toString()
                    .orEmpty()
            updateSearch(query)
        }
    }

    private fun setUpPicker(view: View) {
        recentEmoji = RecentEmojiManager(requireContext())
        variantEmoji = VariantEmojiManager(requireContext())
        binding.emojiPicker.setUp(
            rootView = view,
            onEmojiClickListener = { emoji -> selectEmoji(emoji) },
            onEmojiBackspaceClickListener = null,
            editText = null,
            theming =
                EmojiTheming.from(requireContext()).copy(
                    backgroundColor = Color.TRANSPARENT,
                    primaryColor = MaterialColors.getColor(view, com.google.android.material.R.attr.colorOnSurfaceVariant),
                    secondaryColor = MaterialColors.getColor(view, androidx.appcompat.R.attr.colorPrimary),
                ),
            recentEmoji = recentEmoji,
            searchEmoji = NoSearchEmoji,
            variantEmoji = variantEmoji,
        )
    }

    override fun onStart() {
        super.onStart()
        (dialog as BottomSheetDialog).apply {
            window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
            val sheet = findViewById<View>(com.google.android.material.R.id.design_bottom_sheet)
            sheet?.layoutParams?.height = ViewGroup.LayoutParams.MATCH_PARENT
            behavior.maxHeight = (resources.displayMetrics.heightPixels * 0.85f).toInt()
            behavior.skipCollapsed = true
            behavior.state = BottomSheetBehavior.STATE_EXPANDED
        }
    }

    private fun updateSearch(query: String) {
        if (!pickerReady) return
        searchJob?.cancel()
        val trimmed = query.trim()
        renderSearch(trimmed)
        if (trimmed.length < 2) return

        searchJob =
            viewLifecycleOwner.lifecycleScope.launch {
                val results = withContext(Dispatchers.Default) { search.search(trimmed) }
                resultsAdapter.submitList(results) {
                    val currentBinding = viewBinding ?: return@submitList
                    val currentQuery =
                        currentBinding.emojiSearch.text
                            .toString()
                            .trim()
                    if (currentQuery != trimmed) return@submitList
                    renderSearch(trimmed, results)
                }
            }
    }

    private fun renderSearch(
        query: String,
        results: List<SearchEmojiResult>? = null,
    ) {
        val message =
            when {
                query.length == 1 -> R.string.pin_emoji_search_minimum
                results?.isEmpty() == true -> R.string.pin_emoji_search_empty
                else -> null
            }
        binding.emojiPicker.isVisible = query.isEmpty()
        binding.emojiResults.isVisible = !results.isNullOrEmpty()
        binding.searchStatus.isVisible = message != null
        message?.let { binding.searchStatus.setText(it) }
    }

    private fun selectSearchResult(emoji: Emoji) {
        recentEmoji.addEmoji(emoji)
        variantEmoji.addVariant(emoji)
        selectEmoji(emoji)
    }

    private fun selectEmoji(emoji: Emoji) {
        model.setPinnedEmoji(requireArguments().getString(FILE_ID)!!, emoji.unicode)
        dismiss()
    }

    override fun onDestroyView() {
        if (pickerReady) binding.emojiPicker.tearDown()
        pickerReady = false
        binding.emojiResults.adapter = null
        viewBinding = null
        super.onDestroyView()
    }

    companion object {
        private const val FILE_ID = "file_id"
        private const val FILE_NAME = "file_name"

        fun newInstance(
            fileId: String,
            fileName: String,
        ) = PinEmojiBottomSheetFragment().apply {
            arguments = bundleOf(FILE_ID to fileId, FILE_NAME to fileName)
        }
    }
}

private class PinEmojiSearchAdapter(
    private val onSelect: (Emoji) -> Unit,
) : ListAdapter<SearchEmojiResult, PinEmojiSearchAdapter.Holder>(PinEmojiSearchDiff()) {
    override fun onCreateViewHolder(
        parent: ViewGroup,
        viewType: Int,
    ) = Holder(LayoutInflater.from(parent.context).inflate(R.layout.pin_emoji_search_item, parent, false) as MaterialTextView)

    override fun onBindViewHolder(
        holder: Holder,
        position: Int,
    ) {
        val result = getItem(position)
        holder.text.text = result.emoji.unicode
        holder.text.contentDescription = result.shortcode.replace('_', ' ')
        holder.text.setOnClickListener { onSelect(result.emoji) }
    }

    class Holder(
        val text: MaterialTextView,
    ) : RecyclerView.ViewHolder(text)
}

private class PinEmojiSearchDiff : DiffUtil.ItemCallback<SearchEmojiResult>() {
    override fun areItemsTheSame(
        oldItem: SearchEmojiResult,
        newItem: SearchEmojiResult,
    ) = oldItem.emoji.unicode == newItem.emoji.unicode

    override fun areContentsTheSame(
        oldItem: SearchEmojiResult,
        newItem: SearchEmojiResult,
    ) = oldItem == newItem
}
