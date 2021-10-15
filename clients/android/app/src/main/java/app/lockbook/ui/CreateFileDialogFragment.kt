package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import androidx.fragment.app.DialogFragment
import androidx.fragment.app.activityViewModels
import app.lockbook.App.Companion.config
import app.lockbook.R
import app.lockbook.databinding.DialogCreateFileBinding
import app.lockbook.model.*
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class CreateFileDialogFragment : DialogFragment() {

    private var _binding: DialogCreateFileBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!
    private val createFileCreate get() = binding.createFileCreate
    private val createFileExtension get() = binding.createFileExtension
    private val createFileText get() = binding.createFileText
    private val createFileTextPart get() = binding.createFileTextPart
    private val createFileTitle get() = binding.createFileTitle

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val activityModel: StateViewModel by activityViewModels()
    private lateinit var info: CreateFileInfo
    var newFile: ClientFileMetadata? = null

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    companion object {
        const val CREATE_FILE_DIALOG_TAG = "CreateFileDialogFragment"
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = DialogCreateFileBinding.inflate(
            inflater,
            container,
            false
        )

        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        binding.createFileCancel.setOnClickListener {
            dismiss()
        }

        dialog?.setCanceledOnTouchOutside(false)
            ?: alertModel.notifyBasicError()

        info = (activityModel.transientScreen as TransientScreen.Create).info

        when (info.extendedFileType) {
            ExtendedFileType.Folder -> {
                createFileExtension.visibility = View.GONE
                createFileTextPart.visibility = View.GONE
                createFileText.visibility = View.VISIBLE

                createFileText.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_DONE) {
                        handleCreateFileRequest(createFileText.text.toString())
                    }

                    true
                }

                createFileCreate.setOnClickListener {
                    handleCreateFileRequest(createFileText.text.toString())
                }

                createFileCreate.setOnClickListener {
                    handleCreateFileRequest(createFileText.text.toString())
                }

                createFileTitle.setText(R.string.create_file_title_folder)
                createFileText.setHint(R.string.create_file_hint_folder)
            }
            ExtendedFileType.Text, ExtendedFileType.Drawing -> {
                createFileTextPart.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_NEXT) {
                        createFileExtension.requestFocus()
                        val extension = createFileExtension.text.toString()
                        if (extension.isEmpty()) {
                            createFileExtension.setText(".")
                            createFileExtension.setSelection(1)
                        } else {
                            createFileExtension.setSelection(extension.length)
                        }
                    }

                    true
                }

                createFileExtension.setOnEditorActionListener { _, actionId, _ ->
                    if (actionId == EditorInfo.IME_ACTION_DONE) {
                        handleCreateFileRequest(createFileTextPart.text.toString() + createFileExtension.text.toString())
                    }

                    true
                }

                createFileCreate.setOnClickListener {
                    handleCreateFileRequest(createFileTextPart.text.toString() + createFileExtension.text.toString())
                }

                if (info.extendedFileType == ExtendedFileType.Drawing) {
                    createFileTitle.setText(R.string.create_file_title_drawing)
                    createFileTextPart.setHint(R.string.create_file_hint_drawing)
                    createFileExtension.setHint(R.string.create_file_hint_drawing_extension)
                    createFileExtension.setText(R.string.create_file_dialog_drawing_extension)
                } else {
                    createFileTitle.setText(R.string.create_file_title_document)
                    createFileTextPart.setHint(R.string.create_file_hint_document)
                    createFileExtension.setHint(R.string.create_file_hint_document_extension)
                    createFileExtension.setText(R.string.create_file_dialog_document_extension)
                }
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
                CoreModel.createFile(config, info.parentId, name, info.extendedFileType.toFileType())
        ) {
            is Ok -> {
                newFile = createFileResult.value

                withContext(Dispatchers.Main) {
                    dismiss()
                }
            }
            is Err -> alertModel.notifyError(createFileResult.error.toLbError(resources))
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
