package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.appcompat.app.AlertDialog
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.Messages
import app.lockbook.util.RenameFileError
import app.lockbook.util.exhaustive
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.dialog_move_file.*
import kotlinx.android.synthetic.main.dialog_rename_file.*
import kotlinx.coroutines.*
import timber.log.Timber

class RenameFileDialogFragment : DialogFragment() {

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
    ): View? = inflater.inflate(
        R.layout.dialog_rename_file,
        container,
        false
    )

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        val bundle = arguments
        val nullableId = bundle?.getString(ID_KEY)
        val nullableName = bundle?.getString(NAME_KEY)
        if (nullableId != null && nullableName != null) {
            id = nullableId
            name = nullableName
        } else {
            Snackbar.make(rename_file_layout, Messages.UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT)
                .addCallback(object : Snackbar.Callback() {
                    override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                        super.onDismissed(transientBottomBar, event)
                        dismiss()
                    }
                }).show()
        }
        config = Config(requireNotNull(this.activity).application.filesDir.absolutePath)

        rename_file_cancel.setOnClickListener {
            dismiss()
        }

        rename_file_rename.setOnClickListener {
            handleRenameRequest(rename_file.text.toString())
        }

        rename_file.setText(name)
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
                is RenameFileError.FileDoesNotExist -> errorHasOccurred("Error! File does not exist!")
                is RenameFileError.NewNameContainsSlash -> errorHasOccurred("Error! New name contains slash!")
                is RenameFileError.FileNameNotAvailable -> errorHasOccurred("Error! File name not available!")
                is RenameFileError.NewNameEmpty -> errorHasOccurred("Error! New file name cannot be empty!")
                is RenameFileError.CannotRenameRoot -> errorHasOccurred("Error! Cannot rename root!")
                is RenameFileError.Unexpected -> {
                    Timber.e("Unable to rename file: ${error.error}")
                    unexpectedErrorHasOccurred(
                        error.error
                    )
                }
            }
        }.exhaustive
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.WRAP_CONTENT
        )
    }

    private suspend fun errorHasOccurred(error: String) {
        withContext(Dispatchers.Main) {
            Snackbar.make(rename_file_layout, error, Snackbar.LENGTH_SHORT).show()
        }
    }

    private suspend fun unexpectedErrorHasOccurred(error: String) {
        withContext(Dispatchers.Main) {
            AlertDialog.Builder(requireContext(), R.style.DarkBlue_Dialog)
                .setTitle(Messages.UNEXPECTED_ERROR)
                .setMessage(error)
                .setOnCancelListener {
                    dismiss()
                }
                .show()
        }
    }
}
