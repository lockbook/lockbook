@file:Suppress(
    "ktlint:standard:backing-property-naming",
    "ktlint:standard:no-wildcard-imports",
)

package app.lockbook.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
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
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.core.view.doOnLayout
import androidx.core.view.isVisible
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.SheetShareFileBinding
import app.lockbook.model.FileTreeViewModel
import app.lockbook.screen.MainScreenActivity
import app.lockbook.screen.UpdateFilesUI
import app.lockbook.util.OpenLinkBuilder
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
    private val files: List<File> by lazy {
        requireArguments().getStringArrayList(FILE_IDS_KEY)!!.map(Lb::getFileById)
    }
    private val file: File get() = files.single()
    private val sharedUsernames = linkedSetOf<String>()

    companion object {
        const val TAG = "ShareFileBottomSheetFragment"
        private const val FILE_IDS_KEY = "file_ids"
        private const val INVITE_FORM_VISIBLE_KEY = "invite_form_visible"

        fun newInstance(fileIds: List<String>): ShareFileBottomSheetFragment =
            ShareFileBottomSheetFragment().apply {
                arguments = Bundle().apply { putStringArrayList(FILE_IDS_KEY, ArrayList(fileIds)) }
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
        val singleFile = files.single()
        binding.shareFileName.text = singleFile.name
        fileTreeViewModel.sharingContacts.observe(viewLifecycleOwner) { contacts ->
            sharedUsernames.clear()
            contacts.forEach { addParticipant(it.username) }
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
        binding.shareFileSendLink.setOnClickListener { shareLockbookLink() }
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
        binding.shareFileMainContent.isVisible = false
        binding.shareFileInviteForm.isVisible = true
        binding.shareFileUsername.requestFocus()
    }

    private fun showMainSheet() {
        binding.shareFileInviteForm.isVisible = false
        binding.shareFileMainContent.isVisible = true
        binding.shareFileErrorContainer.isVisible = false
        binding.shareFileUsernameLayout.error = null
        (requireContext().getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager)
            .hideSoftInputFromWindow(binding.shareFileUsername.windowToken, 0)
        binding.shareFileName.requestFocus()
    }

    private fun updateAccessRow() {
        val hasShares = sharedUsernames.isNotEmpty()
        binding.shareFileContactsHeading.isVisible = hasShares
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
        val primaryColor = MaterialColors.getColor(binding.root, androidx.appcompat.R.attr.colorPrimary)
        val labelColor = MaterialColors.getColor(binding.root, com.google.android.material.R.attr.colorOnSurfaceVariant)
        sharedUsernames.forEachIndexed { index, username ->
            val colorIndex = Math.floorMod(username.lowercase(Locale.ROOT).hashCode(), 8)
            val seedColor = Color.HSVToColor(floatArrayOf(colorIndex * 45f, 0.65f, 0.8f))
            val colorRoles = MaterialColors.getColorRoles(requireContext(), MaterialColors.harmonize(seedColor, primaryColor))
            val avatarColor = colorRoles.accentContainer
            val initialColor = colorRoles.onAccentContainer
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
                    addView(avatar, LinearLayout.LayoutParams(avatarSize, avatarSize))
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

    private fun addParticipant(username: String) {
        if (username.isNotBlank() && sharedUsernames.none { it.equals(username, ignoreCase = true) }) {
            sharedUsernames.add(username)
        }
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
                withContext(Dispatchers.Main) {
                    binding.shareFileAddUser.isEnabled = true
                    binding.shareFileBack.isEnabled = true
                    binding.shareFileUsername.text?.clear()
                    fileTreeViewModel.refreshSharingContacts()
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
        val activity = activity as? MainScreenActivity ?: return
        Snackbar
            .make(activity.findViewById(android.R.id.content), message, Snackbar.LENGTH_SHORT)
            .apply {
                if (activity.fileActionSnackbarAnchorView.isShown) {
                    setAnchorView(activity.fileActionSnackbarAnchorView)
                }
                if (offerLink) setAction(R.string.copy_link) { copyLockbookLink() }
            }.show()
    }

    private fun lockbookLink(): String = OpenLinkBuilder.build(Lb.getAccount().apiUrl, file.id)

    private fun copyLockbookLink() {
        runCatching {
            val currentContext = context ?: return
            val clipboard = currentContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(
                ClipData.newPlainText(currentContext.getString(R.string.copy_lockbook_link), lockbookLink()),
            )
            showSuccessSnackbar(getString(R.string.lockbook_link_copied))
        }.onFailure(::showUnexpectedError)
    }

    private fun shareLockbookLink() {
        runCatching {
            val send =
                Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_TEXT, lockbookLink())
                }
            startActivity(Intent.createChooser(send, getString(R.string.send_lockbook_link)))
        }.onFailure(::showUnexpectedError)
    }

    private fun showUnexpectedError(error: Throwable) {
        Timber.e(error, "Unable to share Lockbook link")
        showSuccessSnackbar(getString(R.string.unexpected_error))
    }
}
