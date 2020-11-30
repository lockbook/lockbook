package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import androidx.appcompat.app.AlertDialog
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.dialog_create_file.*
import kotlinx.android.synthetic.main.dialog_move_file.*
import kotlinx.coroutines.*
import timber.log.Timber

class CreateFileDialogFragment : DialogFragment() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    lateinit var parentId: String
    lateinit var fileType: String
    lateinit var config: Config

    companion object {
        const val CREATE_FILE_DIALOG_TAG = "CreateFileDialogFragment"

        private const val PARENT_ID_KEY = "ID_KEY"
        private const val FILE_TYPE_KEY = "FILE_TYPE_KEY"

        fun newInstance(parentId: String, fileType: String): CreateFileDialogFragment {
            val args = Bundle()
            args.putString(PARENT_ID_KEY, parentId)
            args.putString(FILE_TYPE_KEY, fileType)

            val fragment = CreateFileDialogFragment()
            fragment.arguments = args
            return fragment
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? = inflater.inflate(
        R.layout.dialog_create_file,
        container,
        false
    )

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        val bundle = arguments
        val nullableParentId = bundle?.getString(PARENT_ID_KEY)
        val nullableFileType = bundle?.getString(FILE_TYPE_KEY)

        if (nullableParentId != null && nullableFileType != null) {
            parentId = nullableParentId
            fileType = nullableFileType
        } else {
            Snackbar.make(create_file_layout, Messages.UNEXPECTED_CLIENT_ERROR, Snackbar.LENGTH_SHORT)
                .addCallback(object : Snackbar.Callback() {
                    override fun onDismissed(transientBottomBar: Snackbar?, event: Int) {
                        super.onDismissed(transientBottomBar, event)
                        dismiss()
                    }
                }).show()
        }

        config = Config(requireNotNull(this.activity).application.filesDir.absolutePath)

        create_file_cancel.setOnClickListener {
            dismiss()
        }

        create_file_create.setOnClickListener {
            handleCreateFileRequest(create_file_text.text.toString())
        }

        create_file_text.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) {
                handleCreateFileRequest(create_file_text.text.toString())
            }

            true
        }
    }

    private fun handleCreateFileRequest(name: String) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                createFile(name)
            }
        }
    }

    private suspend fun createFile(name: String) {
        when (
            val createFileResult =
                CoreModel.createFile(config, parentId, name, fileType)
        ) {
            is Ok -> {
                when (val insertFileResult = CoreModel.insertFile(config, createFileResult.value)) {
                    is Ok -> {
                        withContext(Dispatchers.Main) {
                            dismiss()
                        }
                        return
                    }
                    is Err -> when (val error = insertFileResult.error) {
                        is InsertFileError.Unexpected -> {
                            Timber.e("Unable to insert a newly created file: ${insertFileResult.error}")
                            unexpectedErrorHasOccurred(error.error)
                        }
                    }
                }
            }
            is Err -> when (val error = createFileResult.error) {
                is CreateFileError.NoAccount -> errorHasOccurred("Error! No account!")
                is CreateFileError.DocumentTreatedAsFolder -> errorHasOccurred("Error! Document is treated as folder!")
                is CreateFileError.CouldNotFindAParent -> errorHasOccurred("Error! Could not find file parent!")
                is CreateFileError.FileNameNotAvailable -> errorHasOccurred("Error! File name not available!")
                is CreateFileError.FileNameContainsSlash -> errorHasOccurred("Error! File contains a slash!")
                is CreateFileError.FileNameEmpty -> errorHasOccurred("Error! File cannot be empty!")
                is CreateFileError.Unexpected -> {
                    Timber.e("Unable to create a file: ${error.error}")
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
            Snackbar.make(create_file_layout, error, Snackbar.LENGTH_SHORT).show()
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
