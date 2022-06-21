package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.databinding.DialogRenameFileBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.StateViewModel
import app.lockbook.model.TransientScreen
import app.lockbook.util.LbErrorKind
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

data class RenameFileInfo(
    val id: String,
    val name: String
)

class RenameFileDialogFragment : DialogFragment() {

    private var _binding: DialogRenameFileBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val model: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    companion object {

        const val RENAME_FILE_DIALOG_TAG = "RenameFileDialogFragment"

        fun newInstance(): RenameFileDialogFragment {
            return RenameFileDialogFragment()
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogRenameFileBinding.inflate(
            inflater,
            container,
            false
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        dialog?.setCanceledOnTouchOutside(false) ?: alertModel.notifyBasicError()

        binding.renameFileCancel.setOnClickListener {
            dismiss()
        }

        binding.renameFileRename.setOnClickListener {
            handleRenameRequest(binding.renameFile.text.toString())
        }

        binding.renameFile.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                handleRenameRequest(binding.renameFile.text.toString())
            }

            true
        }

        binding.renameFile.setText((model.transientScreen as TransientScreen.Rename).file.decryptedName)
    }

    private fun handleRenameRequest(newName: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                renameFile(newName)
            }
        }
    }

    private suspend fun renameFile(newName: String) {
        when (val renameFileResult = CoreModel.renameFile((model.transientScreen as TransientScreen.Rename).file.id, newName)) {
            is Ok -> {
                withContext(Dispatchers.Main) {
                    dismiss()
                }
                return
            }
            is Err -> withContext(Dispatchers.Main) {
                val lbError = renameFileResult.error.toLbError(resources)
                alertModel.notifyError(lbError, if (lbError.kind == LbErrorKind.Program) ::dismiss else null)
            }
        }
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            (resources.displayMetrics.widthPixels * 0.9).toInt(),
            WindowManager.LayoutParams.WRAP_CONTENT
        )
    }
}
