package app.lockbook.loggedin.listfiles

import android.content.Intent
import android.os.Bundle
import android.view.LayoutInflater
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup
import android.widget.Toast
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.loggedin.newfile.NewFileActivity
import app.lockbook.loggedin.popupinfo.PopUpInfoActivity
import app.lockbook.loggedin.texteditor.TextEditorActivity
import app.lockbook.utils.EditableFile
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.RequestResultCodes.NEW_FILE_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE
import kotlinx.android.synthetic.main.fragment_list_files.*
import timber.log.Timber

class ListFilesFragment : Fragment() {

    private lateinit var listFilesViewModel: ListFilesViewModel

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val binding: FragmentListFilesBinding = DataBindingUtil.inflate(
            inflater, R.layout.fragment_list_files, container, false
        )
        val application = requireNotNull(this.activity).application
        val listFilesViewModelFactory =
            ListFilesViewModelFactory(application.filesDir.absolutePath, application)
        listFilesViewModel =
            ViewModelProvider(this, listFilesViewModelFactory).get(ListFilesViewModel::class.java)
        val adapter =
            FilesAdapter(listFilesViewModel)

        binding.listFilesViewModel = listFilesViewModel
        binding.filesList.adapter = adapter
        binding.filesList.layoutManager = LinearLayoutManager(context)
        binding.lifecycleOwner = this

        binding.listFilesRefresh.setOnRefreshListener {
            listFilesViewModel.syncRefresh()
        }

        listFilesViewModel.files.observe(
            viewLifecycleOwner,
            Observer { files ->
                updateRecyclerView(files, adapter)
            }
        )

        listFilesViewModel.navigateToFileEditor.observe(
            viewLifecycleOwner,
            Observer { editableFile ->
                navigateToFileEditor(editableFile)
            }
        )

        listFilesViewModel.navigateToPopUpInfo.observe(
            viewLifecycleOwner,
            Observer { fileMetadata ->
                navigateToPopUpInfo(fileMetadata)
            }
        )

        listFilesViewModel.navigateToNewFile.observe(
            viewLifecycleOwner,
            Observer {
                navigateToNewFile()
            }
        )

        listFilesViewModel.listFilesRefreshing.observe(
            viewLifecycleOwner,
            Observer { isRefreshing ->
                list_files_refresh.isRefreshing = isRefreshing
            }
        )

        listFilesViewModel.errorHasOccurred.observe(
            viewLifecycleOwner,
            Observer { errorText ->
                errorHasOccurred(errorText)
            }
        )

        listFilesViewModel.startUpFiles()

        return binding.root
    }

    private fun updateRecyclerView(
        files: List<FileMetadata>,
        adapter: FilesAdapter
    ) {
        if (files.isEmpty()) {
            adapter.files = listOf()
        } else {
            adapter.files = files
        }
    }

    private fun navigateToFileEditor(editableFile: EditableFile) {
        val intent = Intent(context, TextEditorActivity::class.java)
        intent.putExtra("name", editableFile.name)
        intent.putExtra("contents", editableFile.contents)
        startActivityForResult(intent, TEXT_EDITOR_REQUEST_CODE)
    }

    private fun navigateToPopUpInfo(fileMetadata: FileMetadata) {
        val intent = Intent(context, PopUpInfoActivity::class.java)
        intent.putExtra("name", fileMetadata.name)
        intent.putExtra("id", fileMetadata.id)
        intent.putExtra("fileType", fileMetadata.file_type.toString())
        intent.putExtra("metadataVersion", fileMetadata.metadata_version.toString())
        intent.putExtra("contentVersion", fileMetadata.content_version.toString())
        startActivityForResult(intent, POP_UP_INFO_REQUEST_CODE)
    }

    private fun navigateToNewFile() {
        val intent = Intent(context, NewFileActivity::class.java)
        startActivityForResult(intent, NEW_FILE_REQUEST_CODE)
    }

    private fun errorHasOccurred(errorText: String) {
        Toast.makeText(context, errorText, Toast.LENGTH_LONG).show()
    }

    fun onBackPressed(): Boolean {
        return listFilesViewModel.quitOrNot()
    }

    fun onSortPressed(id: Int) {
        Timber.i("HERE2")
        listFilesViewModel.onSortPressed(id)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        listFilesViewModel.handleActivityResult(requestCode, resultCode, data)
    }
}
