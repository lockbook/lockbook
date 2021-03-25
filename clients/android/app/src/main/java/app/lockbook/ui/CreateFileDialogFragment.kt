package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import androidx.fragment.app.DialogFragment
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.dialog_create_file.*
import kotlinx.coroutines.*
import timber.log.Timber
import kotlin.properties.Delegates

data class CreateFileInfo(
    val parentId: String,
    val fileType: String,
    val isDrawing: Boolean
)

class CreateFileDialogFragment : DialogFragment() {

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)
    private lateinit var parentId: String
    private lateinit var fileType: String
    private var isDrawing by Delegates.notNull<Boolean>()
    var newDocument: FileMetadata? = null
    lateinit var config: Config

    companion object {
        const val CREATE_FILE_DIALOG_TAG = "CreateFileDialogFragment"

        private const val PARENT_ID_KEY = "ID_KEY"
        private const val FILE_TYPE_KEY = "FILE_TYPE_KEY"
        private const val IS_DRAWING_KEY = "IS_DRAWING_KEY"

        fun newInstance(parentId: String, fileType: String, isDrawing: Boolean): CreateFileDialogFragment {
            val args = Bundle()
            args.putString(PARENT_ID_KEY, parentId)
            args.putString(FILE_TYPE_KEY, fileType)
            args.putBoolean(IS_DRAWING_KEY, isDrawing)

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
        val nullableIsDrawing = bundle?.getBoolean(IS_DRAWING_KEY)

        if (nullableParentId != null && nullableFileType != null && nullableIsDrawing != null) {
            parentId = nullableParentId
            fileType = nullableFileType
            isDrawing = nullableIsDrawing
        } else {
            AlertModel.errorHasOccurred(create_file_layout, BASIC_ERROR, OnFinishAlert.DoSomethingOnFinishAlert(::dismiss))
        }

        config = Config(requireNotNull(this.activity).application.filesDir.absolutePath)

        create_file_cancel.setOnClickListener {
            dismiss()
        }

        dialog?.setCanceledOnTouchOutside(false)
            ?: AlertModel.errorHasOccurred(create_file_layout, BASIC_ERROR, OnFinishAlert.DoNothingOnFinishAlert)

        when (fileType) {
            Klaxon().toJsonString(FileType.Folder) -> {
                create_file_extension.visibility = View.GONE
                create_file_text_part.visibility = View.GONE
                create_file_text.visibility = View.VISIBLE

                create_file_text.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_DONE) {
                        handleCreateFileRequest(create_file_text.text.toString())
                    }

                    true
                }

                create_file_create.setOnClickListener {
                    handleCreateFileRequest(create_file_text.text.toString())
                }

                create_file_create.setOnClickListener {
                    handleCreateFileRequest(create_file_text.text.toString())
                }

                create_file_title.setText(R.string.create_file_title_folder)
                create_file_text.setHint(R.string.create_file_hint_folder)
            }
            Klaxon().toJsonString(FileType.Document) -> {
                create_file_text_part.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_NEXT) {
                        create_file_extension.requestFocus()
                        val extension = create_file_extension.text.toString()
                        if (extension.isEmpty()) {
                            create_file_extension.setText(".")
                            create_file_extension.setSelection(1)
                        } else {
                            create_file_extension.setSelection(extension.length)
                        }
                    }

                    true
                }

                create_file_extension.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_DONE) {
                        handleCreateFileRequest(create_file_text_part.text.toString() + create_file_extension.text.toString())
                    }

                    true
                }

                create_file_create.setOnClickListener {
                    handleCreateFileRequest(create_file_text_part.text.toString() + create_file_extension.text.toString())
                }

                if (isDrawing) {
                    create_file_title.setText(R.string.create_file_title_drawing)
                    create_file_text_part.setHint(R.string.create_file_hint_drawing)
                    create_file_extension.setHint(R.string.create_file_hint_drawing_extension)
                    create_file_extension.setText(R.string.create_file_dialog_drawing_extension)
                } else {
                    create_file_title.setText(R.string.create_file_title_document)
                    create_file_text_part.setHint(R.string.create_file_hint_document)
                    create_file_extension.setHint(R.string.create_file_hint_document_extension)
                    create_file_extension.setText(R.string.create_file_dialog_document_extension)
                }
            }
            else -> {
                AlertModel.errorHasOccurred(create_file_layout, BASIC_ERROR, OnFinishAlert.DoSomethingOnFinishAlert(::dismiss))
            }
        }.exhaustive
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
                        if (fileType == Klaxon().toJsonString(FileType.Document)) {
                            newDocument = createFileResult.value
                        }
                        withContext(Dispatchers.Main) {
                            dismiss()
                        }
                    }
                    is Err -> when (val error = insertFileResult.error) {
                        is InsertFileError.Unexpected -> {
                            Timber.e("Unable to insert a newly created file: ${insertFileResult.error}")
                            withContext(Dispatchers.Main) {
                                AlertModel.unexpectedCoreErrorHasOccurred(
                                    requireContext(),
                                    error.error,
                                    OnFinishAlert.DoSomethingOnFinishAlert(::dismiss)
                                )
                            }
                        }
                    }
                }
            }
            is Err -> when (val error = createFileResult.error) {
                is CreateFileError.NoAccount -> AlertModel.errorHasOccurred(create_file_layout, "Error! No account!", OnFinishAlert.DoNothingOnFinishAlert)
                is CreateFileError.DocumentTreatedAsFolder -> AlertModel.errorHasOccurred(create_file_layout, "Error! Document is treated as folder!", OnFinishAlert.DoNothingOnFinishAlert)
                is CreateFileError.CouldNotFindAParent -> AlertModel.errorHasOccurred(create_file_layout, "Error! Could not find file parent!", OnFinishAlert.DoNothingOnFinishAlert)
                is CreateFileError.FileNameNotAvailable -> AlertModel.errorHasOccurred(create_file_layout, "Error! File name not available!", OnFinishAlert.DoNothingOnFinishAlert)
                is CreateFileError.FileNameContainsSlash -> AlertModel.errorHasOccurred(create_file_layout, "Error! File contains a slash!", OnFinishAlert.DoNothingOnFinishAlert)
                is CreateFileError.FileNameEmpty -> AlertModel.errorHasOccurred(create_file_layout, "Error! File cannot be empty!", OnFinishAlert.DoNothingOnFinishAlert)
                is CreateFileError.Unexpected -> {
                    Timber.e("Unable to create a file: ${error.error}")
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
