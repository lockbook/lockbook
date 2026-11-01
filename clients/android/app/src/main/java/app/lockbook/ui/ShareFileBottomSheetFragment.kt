@file:Suppress(
    "ktlint:standard:backing-property-naming",
    "ktlint:standard:no-wildcard-imports",
)

package app.lockbook.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.text.TextUtils
import android.view.Gravity
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.core.graphics.ColorUtils
import androidx.core.view.doOnLayout
import androidx.core.view.isVisible
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.SheetShareFileBinding
import app.lockbook.model.FileTreeViewModel
import app.lockbook.screen.UpdateFilesUI
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.bottomsheet.BottomSheetDialogFragment
import com.google.android.material.color.MaterialColors
import com.google.android.material.snackbar.Snackbar
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.File
import net.lockbook.File.ShareMode
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.LbError.LbEC
import timber.log.Timber
import java.util.Locale

class ShareFileBottomSheetFragment : BottomSheetDialogFragment() {
    private var _binding: SheetShareFileBinding? = null
    private val binding get() = _binding!!
    private val fileTreeViewModel: FileTreeViewModel by activityViewModels()
    private val file: File by lazy { Lb.getFileById(requireArguments().getString(FILE_ID_KEY)!!) }
    private var sharedUsernames: List<String> = emptyList()

    companion object {
        const val TAG = "ShareFileBottomSheetFragment"
        private const val FILE_ID_KEY = "file_id"
        private const val INVITE_FORM_VISIBLE_KEY = "invite_form_visible"

        fun newInstance(fileId: String): ShareFileBottomSheetFragment =
            ShareFileBottomSheetFragment().apply {
                arguments = Bundle().apply { putString(FILE_ID_KEY, fileId) }
            }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = SheetShareFileBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)
        binding.shareFileName.text = file.name
        binding.shareFileInviteName.text = file.name
        fileTreeViewModel.sharingContacts.observe(viewLifecycleOwner) { contacts ->
            sharedUsernames = contacts.map { it.username }
            updateAccessRow()
        }
        binding.shareFileAccessPeople.doOnLayout { updateAccessRow() }
        binding.shareFileAddPerson.setOnClickListener { showInviteForm() }
        binding.shareFileAddFirstPerson.setOnClickListener { showInviteForm() }
        binding.shareFileBack.setOnClickListener { showMainSheet() }
        (requireDialog() as BottomSheetDialog).onBackPressedDispatcher.addCallback(
            viewLifecycleOwner,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (binding.shareFileInviteForm.isVisible) {
                        if (binding.shareFileBack.isEnabled) showMainSheet()
                    } else {
                        dismiss()
                    }
                }
            },
        )
        binding.shareFileAccessMode.setText(getString(R.string.share_mode_read), false)
        binding.shareFileAccessMode.setOnItemClickListener { _, _, _, _ ->
            binding.shareFileErrorContainer.isVisible = false
        }
        binding.shareFileAddUser.setOnClickListener { shareFile() }
        binding.shareFileUsername.doAfterTextChanged {
            binding.shareFileUsernameLayout.error = null
            binding.shareFileErrorContainer.isVisible = false
        }
        binding.shareFileUsername.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                shareFile()
                true
            } else {
                false
            }
        }
        binding.shareFileCopyLink.setOnClickListener { copyLockbookLink() }
        if (savedInstanceState?.getBoolean(INVITE_FORM_VISIBLE_KEY) == true) showInviteForm(clearUsername = false)
    }

    override fun onStart() {
        super.onStart()
        (dialog as? BottomSheetDialog)?.behavior?.apply {
            state = BottomSheetBehavior.STATE_EXPANDED
            skipCollapsed = true
        }
    }

    override fun onDestroyView() {
        _binding = null
        super.onDestroyView()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putBoolean(INVITE_FORM_VISIBLE_KEY, _binding?.shareFileInviteForm?.isVisible == true)
        super.onSaveInstanceState(outState)
    }

    private fun showInviteForm(
        username: String = "",
        clearUsername: Boolean = true,
    ) {
        if (clearUsername) {
            binding.shareFileUsername.setText(username)
            binding.shareFileUsername.setSelection(binding.shareFileUsername.length())
        }
        binding.shareFileMainContent.visibility = View.INVISIBLE
        binding.shareFileInviteForm.isVisible = true
        binding.shareFileUsername.requestFocus()
    }

    private fun showMainSheet() {
        binding.shareFileInviteForm.visibility = View.INVISIBLE
        binding.shareFileMainContent.isVisible = true
        binding.shareFileErrorContainer.isVisible = false
        binding.shareFileUsernameLayout.error = null
        (requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
            .hideSoftInputFromWindow(binding.shareFileUsername.windowToken, 0)
        binding.shareFileName.requestFocus()
    }

    private fun updateAccessRow() {
        val hasShares = sharedUsernames.isNotEmpty()
        binding.shareFileAccessEmpty.isVisible = !hasShares
        binding.shareFileAccessPeople.isVisible = hasShares
        binding.shareFileAvatarList.removeAllViews()
        if (!hasShares) return

        val avatarSize = (56 * resources.displayMetrics.density).toInt()
        val avatarSpacing = (4 * resources.displayMetrics.density).toInt()
        val labelSpacing = (4 * resources.displayMetrics.density).toInt()
        val minLabelWidth = (64 * resources.displayMetrics.density).toInt()
        val maxLabelWidth = (72 * resources.displayMetrics.density).toInt()
        val addButtonWidth =
            binding.shareFileAddPerson.width.takeIf { it > 0 } ?: avatarSize
        val availableWidth =
            (binding.shareFileAccessPeople.width - addButtonWidth - avatarSpacing).coerceAtLeast(minLabelWidth)
        val labelWidth = (availableWidth / sharedUsernames.size).coerceIn(minLabelWidth, maxLabelWidth)
        val unshareSize = (24 * resources.displayMetrics.density).toInt()
        val accessUsernames = Lb.getFileById(file.id).shares.mapTo(mutableSetOf()) { it.sharedWith.lowercase(Locale.ROOT) }
        val primaryColor = MaterialColors.getColor(binding.root, androidx.appcompat.R.attr.colorPrimary)
        val backgroundColors =
            intArrayOf(
                MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorSurface),
                MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorSurfaceContainerLow),
            )
        sharedUsernames.forEachIndexed { index, username ->
            val colorIndex = Math.floorMod(username.lowercase(Locale.ROOT).hashCode(), 8)
            val seedColor = Color.HSVToColor(floatArrayOf(colorIndex * 45f, 0.65f, 0.8f))
            val colorRoles = MaterialColors.getColorRoles(requireContext(), MaterialColors.harmonize(seedColor, primaryColor))
            val avatarColor = colorRoles.accentContainer
            val initialColor = colorRoles.onAccentContainer
            val labelColor = readableUsernameColor(colorRoles.accent, backgroundColors)
            val avatar =
                TextView(requireContext()).apply {
                    text = username.take(1).uppercase()
                    contentDescription = username
                    gravity = Gravity.CENTER
                    setTextColor(initialColor)
                    textSize = 21f
                    background =
                        GradientDrawable().apply {
                            shape = GradientDrawable.OVAL
                            setColor(avatarColor)
                        }
                }
            val label =
                TextView(requireContext()).apply {
                    text = username
                    gravity = Gravity.CENTER
                    maxLines = 1
                    ellipsize = TextUtils.TruncateAt.END
                    setTextColor(labelColor)
                    textSize = 12f
                }
            val person =
                LinearLayout(requireContext()).apply {
                    gravity = Gravity.CENTER_HORIZONTAL
                    orientation = LinearLayout.VERTICAL
                    contentDescription = getString(R.string.share_with_named_user, username)
                    isClickable = true
                    isFocusable = true
                    setOnClickListener { showInviteForm(username) }
                    addView(
                        FrameLayout(requireContext()).apply {
                            addView(avatar, FrameLayout.LayoutParams(avatarSize, avatarSize))
                            if (username.lowercase(Locale.ROOT) in accessUsernames) {
                                addView(
                                    unshareButton(username),
                                    FrameLayout.LayoutParams(unshareSize, unshareSize, Gravity.TOP or Gravity.END),
                                )
                            }
                        },
                        LinearLayout.LayoutParams(avatarSize, avatarSize),
                    )
                    addView(
                        label,
                        LinearLayout.LayoutParams(labelWidth, LinearLayout.LayoutParams.WRAP_CONTENT).apply {
                            topMargin = labelSpacing
                        },
                    )
                }
            binding.shareFileAvatarList.addView(
                person,
                LinearLayout.LayoutParams(labelWidth, LinearLayout.LayoutParams.WRAP_CONTENT).apply {
                    if (index > 0) marginStart = avatarSpacing
                },
            )
        }
    }

    private fun unshareButton(username: String): ImageView {
        val padding = (4 * resources.displayMetrics.density).toInt()
        return ImageView(requireContext()).apply {
            setImageResource(R.drawable.ic_outline_delete_24)
            imageTintList =
                ColorStateList.valueOf(
                    MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorOnErrorContainer),
                )
            background =
                GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorErrorContainer))
                }
            setPadding(padding, padding, padding, padding)
            contentDescription = getString(R.string.unshare_with_named_user, username)
            isClickable = true
            isFocusable = true
            setOnClickListener { unshareFile(username) }
        }
    }

    private fun readableUsernameColor(
        accent: Int,
        backgrounds: IntArray,
    ): Int {
        if (backgrounds.all { ColorUtils.calculateContrast(accent, it) >= 4.5 }) return accent

        val dark = backgrounds.minOf { ColorUtils.calculateContrast(Color.BLACK, it) }
        val light = backgrounds.minOf { ColorUtils.calculateContrast(Color.WHITE, it) }
        val target = if (dark > light) Color.BLACK else Color.WHITE
        var low = 0f
        var high = 1f
        var readable = target
        repeat(12) {
            val blend = (low + high) / 2f
            val candidate = ColorUtils.blendARGB(accent, target, blend)
            if (backgrounds.all { ColorUtils.calculateContrast(candidate, it) >= 4.5 }) {
                readable = candidate
                high = blend
            } else {
                low = blend
            }
        }
        return readable
    }

    private fun shareFile() {
        if (!binding.shareFileAddUser.isEnabled) return

        val username =
            binding.shareFileUsername.text
                ?.toString()
                ?.trim()
                .orEmpty()
        if (username.isEmpty()) {
            binding.shareFileUsernameLayout.error = getString(R.string.no_username)
            return
        }
        binding.shareFileUsernameLayout.error = null
        binding.shareFileErrorContainer.isVisible = false

        val mode =
            when (binding.shareFileAccessMode.text.toString()) {
                getString(R.string.share_mode_write) -> ShareMode.Write
                else -> ShareMode.Read
            }

        binding.shareFileAddUser.isEnabled = false
        binding.shareFileBack.isEnabled = false
        viewLifecycleOwner.lifecycleScope.launch(Dispatchers.IO) {
            try {
                Lb.shareFile(file.id, username, mode == ShareMode.Write)
                fileTreeViewModel._notifyUpdateFilesUI.postValue(UpdateFilesUI.RequestSync)
                fileTreeViewModel.loadSharingContacts()
                withContext(Dispatchers.Main) {
                    binding.shareFileAddUser.isEnabled = true
                    binding.shareFileBack.isEnabled = true
                    binding.shareFileUsername.text?.clear()
                    showMainSheet()
                    showSuccessSnackbar(getString(R.string.shared_with, username), offerLink = true)
                }
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
                    binding.shareFileAddUser.isEnabled = true
                    binding.shareFileBack.isEnabled = true
                    showError(err)
                }
            }
        }
    }

    private fun unshareFile(username: String) {
        viewLifecycleOwner.lifecycleScope.launch(Dispatchers.IO) {
            try {
                Lb.unshareFile(file.id, username)
                fileTreeViewModel._notifyUpdateFilesUI.postValue(UpdateFilesUI.RequestSync)
                fileTreeViewModel.loadSharingContacts()
                withContext(Dispatchers.Main) {
                    showSuccessSnackbar(getString(R.string.unshared_with, username))
                }
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
                    showSuccessSnackbar(if (err.kind == LbEC.Unexpected) getString(R.string.unexpected_error) else err.msg)
                }
            }
        }
    }

    private fun showError(error: LbError) {
        binding.shareFileErrorMessage.text =
            if (error.kind == LbEC.Unexpected) {
                Timber.e("Unexpected error sharing file: %s\n%s", error.msg, error.trace)
                getString(R.string.unexpected_error)
            } else {
                error.msg
            }
        binding.shareFileErrorContainer.isVisible = true
    }

    private fun showSuccessSnackbar(
        message: String,
        offerLink: Boolean = false,
    ) {
        Snackbar
            .make(binding.root, message, Snackbar.LENGTH_SHORT)
            .apply {
                if (offerLink) setAction(R.string.copy_link) { copyLockbookLink() }
            }.show()
    }

    private fun copyLockbookLink() {
        runCatching {
            val currentContext = context ?: return
            val clipboard = currentContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(
                ClipData.newPlainText(currentContext.getString(R.string.copy_lockbook_link), Lb.getFileLinkUrl(file.id)),
            )
        }.onFailure(::showUnexpectedError)
    }

    private fun showUnexpectedError(error: Throwable) {
        Timber.e(error, "Unable to share Lockbook link")
        showSuccessSnackbar(getString(R.string.unexpected_error))
    }
}
