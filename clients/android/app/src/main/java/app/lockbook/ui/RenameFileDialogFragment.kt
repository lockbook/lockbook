package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import app.lockbook.databinding.DialogRenameFileBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.BASIC_ERROR
import app.lockbook.util.Config
import app.lockbook.util.RenameFileError
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import timber.log.Timber

data class RenameFileInfo(
    val id: String,
    val name: String
)

class RenameFileDialogFragment : DialogFragment() {

    private var _binding: DialogRenameFileBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    lateinit var name: String
    lateinit var id: String
    lateinit var config: Config

    companion object {

        const val RENAME_FILE_DIALOG_TAG = "RenameFileDialogFragment"

        private const val ID_KEY = "ID_KEY"
        private const val NAME_KEY = "NAME_KEY"

        fun newInstance(id: String, name: String): RenameFileDialogFragment {
            val args = Bundle()
            args.putString(ID_KEY, id)
            args.putString(NAME_KEY, name)

            val fragment = RenameFileDialogFragment()
            fragment.arguments = args
            return fragment
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DataBindingUtil.inflate(
            inflater,
            R.layout.dialog_rename_file,
            container,
            false
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        val bundle = arguments
        val nullableId = bundle?.getString(ID_KEY)
        val nullableName = bundle?.getString(NAME_KEY)
        if (nullableId != null && nullableName != null) {
            id = nullableId
            name = nullableName
        } else {
            AlertModel.errorHasOccurred(binding.renameFileLayout, BASIC_ERROR, OnFinishAlert.DoSomethingOnFinishAlert(::dismiss))
        }
        config = Config(requireNotNull(this.activity).application.filesDir.absolutePath)
        dialog?.setCanceledOnTouchOutside(false) ?: AlertModel.errorHasOccurred(binding.renameFileLayout, BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)

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

        binding.renameFile.setText(name)
    }

    private fun handleRenameRequest(newName: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                renameFile(newName)
            }
        }
    }

    private suspend fun renameFile(newName: String) {
        when (val renameFileResult = CoreModel.renameFile(config, id, newName)) {
            is Ok -> {
                withContext(Dispatchers.Main) {
                    dismiss()
                }
                return
            }
            is Err -> when (val error = renameFileResult.error) {
                is RenameFileError.FileDoesNotExist -> AlertModel.errorHasOccurred(binding.renameFileLayout, "Error! File does not exist!", OnFinishAlert.DoNothingOnFinishAlert)
                is RenameFileError.NewNameContainsSlash -> AlertModel.errorHasOccurred(binding.renameFileLayout, "Error! New name contains slash!", OnFinishAlert.DoNothingOnFinishAlert)
                is RenameFileError.FileNameNotAvailable -> AlertModel.errorHasOccurred(binding.renameFileLayout, "Error! File name not available!", OnFinishAlert.DoNothingOnFinishAlert)
                is RenameFileError.NewNameEmpty -> AlertModel.errorHasOccurred(binding.renameFileLayout, "Error! New file name cannot be empty!", OnFinishAlert.DoNothingOnFinishAlert)
                is RenameFileError.CannotRenameRoot -> AlertModel.errorHasOccurred(binding.renameFileLayout, "Error! Cannot rename root!", OnFinishAlert.DoNothingOnFinishAlert)
                is RenameFileError.Unexpected -> {
                    Timber.e("Unable to rename file: ${error.error}")
                    withContext(Dispatchers.Main) {
                        AlertModel.unexpectedCoreErrorHasOccurred(
                            requireContext(),
                            error.error,
                            OnFinishAlert.DoSomethingOnFinishAlert(::dismiss)
                        )
                    }
                }
            }
        }.exhaustive
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            (resources.displayMetrics.widthPixels * 0.9).toInt(),
            WindowManager.LayoutParams.WRAP_CONTENT
        )
    }
}
