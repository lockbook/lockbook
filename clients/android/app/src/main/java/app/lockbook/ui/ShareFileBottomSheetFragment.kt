@file:Suppress(
    "ktlint:standard:backing-property-naming",
    "ktlint:standard:no-wildcard-imports",
)

package app.lockbook.ui

import android.content.DialogInterface
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import androidx.core.view.isEmpty
import androidx.core.view.isVisible
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.databinding.SheetShareFileBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.FileTreeViewModel
import app.lockbook.screen.MainScreenActivity
import app.lockbook.screen.UpdateFilesUI
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.bottomsheet.BottomSheetDialogFragment
import com.google.android.material.chip.Chip
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

class ShareFileBottomSheetFragment : BottomSheetDialogFragment() {
    private var _binding: SheetShareFileBinding? = null
    private val binding get() = _binding!!
    private val fileTreeViewModel: FileTreeViewModel by activityViewModels()
    private val file: File by lazy {
        Lb.getFileById(requireArguments().getString(FILE_ID_KEY)!!)
    }
    private var successMessage: String? = null

    companion object {
        const val TAG = "ShareFileBottomSheetFragment"
        private const val FILE_ID_KEY = "file_id"

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
        populateShares()
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

    override fun onDismiss(dialog: DialogInterface) {
        super.onDismiss(dialog)
        successMessage?.let(::showSuccessSnackbar)
        successMessage = null
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
        viewLifecycleOwner.lifecycleScope.launch(Dispatchers.IO) {
            try {
                Lb.shareFile(file.id, username, mode == ShareMode.Write)
                fileTreeViewModel._notifyUpdateFilesUI.postValue(UpdateFilesUI.RequestSync)
                withContext(Dispatchers.Main) {
                    successMessage = getString(R.string.shared_with, username)
                    dismiss()
                }
            } catch (err: LbError) {
                withContext(Dispatchers.Main) {
                    binding.shareFileAddUser.isEnabled = true
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

    private fun showSuccessSnackbar(message: String) {
        val activity = activity as? MainScreenActivity ?: return
        Snackbar
            .make(activity.findViewById(android.R.id.content), message, Snackbar.LENGTH_SHORT)
            .apply {
                if (activity.fileActionSnackbarAnchorView.isShown) {
                    setAnchorView(activity.fileActionSnackbarAnchorView)
                }
            }.show()
    }

    private fun populateShares() {
        for (share in file.shares) {
            val chipGroup =
                when (share.mode) {
                    ShareMode.Write -> binding.shareFileWriteAccessShares
                    ShareMode.Read -> binding.shareFileReadAccessShares
                }

            val chip = createShareChip(share.sharedWith)

            chipGroup.addView(chip)
        }
        binding.shareFileReadAccessGroup.isVisible = !binding.shareFileReadAccessShares.isEmpty()
        binding.shareFileWriteAccessGroup.isVisible = !binding.shareFileWriteAccessShares.isEmpty()
        binding.shareFileExistingAccess.isVisible = file.shares.isNotEmpty()
    }

    private fun createShareChip(username: String): Chip =
        (
            LayoutInflater
                .from(requireActivity())
                .inflate(R.layout.chip_share, null) as Chip
        ).apply {
            text = username
        }
}
