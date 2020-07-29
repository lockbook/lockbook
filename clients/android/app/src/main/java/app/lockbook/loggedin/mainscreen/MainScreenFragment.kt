package app.lockbook.loggedin.mainscreen

import android.content.Intent
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.Toast
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentMainScreenBinding
import app.lockbook.loggedin.listfiles.FilesAdapter
import app.lockbook.loggedin.newfile.NewFileActivity
import app.lockbook.loggedin.popupinfo.PopUpInfoActivity
import app.lockbook.loggedin.texteditor.TextEditorActivity
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.RequestResultCodes.NEW_FILE_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.POP_UP_INFO_REQUEST_CODE
import app.lockbook.utils.RequestResultCodes.TEXT_EDITOR_REQUEST_CODE

class MainScreenFragment : Fragment() {

    lateinit var mainScreenViewModel: MainScreenViewModel

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val binding: FragmentMainScreenBinding = DataBindingUtil.inflate(
            inflater, R.layout.fragment_main_screen, container, false
        )
        val application = requireNotNull(this.activity).application
        val mainScreenViewModelFactory =
            MainScreenViewModelFactory(application.filesDir.absolutePath)
        mainScreenViewModel =
            ViewModelProvider(this, mainScreenViewModelFactory).get(MainScreenViewModel::class.java)
        val adapter = FilesAdapter(mainScreenViewModel)

        binding.mainScreenViewModel = mainScreenViewModel
        binding.filesList.adapter = adapter
        binding.filesList.layoutManager = LinearLayoutManager(context)
        binding.lifecycleOwner = this

        binding.filesListRefresh.setOnRefreshListener {
            mainScreenViewModel.refreshFiles()
            mainScreenViewModel.sync()
            binding.filesListRefresh.isRefreshing = false
        }

        mainScreenViewModel.files.observe(
            viewLifecycleOwner,
            Observer { files ->
                updateRecyclerView(files, adapter)
            }
        )

        mainScreenViewModel.navigateToFileEditor.observe(
            viewLifecycleOwner,
            Observer { fileContents ->
                navigateToFileEditor(fileContents)
            }
        )

        mainScreenViewModel.navigateToPopUpInfo.observe(
            viewLifecycleOwner,
            Observer { fileMetadata ->
                navigateToPopUpInfo(fileMetadata)
            }
        )

        mainScreenViewModel.navigateToNewFile.observe(
            viewLifecycleOwner,
            Observer {
                navigateToNewFile()
            }
        )

        mainScreenViewModel.errorHasOccurred.observe(
            viewLifecycleOwner,
            Observer { errorText ->
                errorHasOccurred(errorText)
            }
        )

        mainScreenViewModel.startUpFiles()

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

    private fun navigateToFileEditor(fileContents: String) {
        val intent = Intent(context, TextEditorActivity::class.java)
        intent.putExtra("text", fileContents)
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
        return mainScreenViewModel.quitOrNot()
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        mainScreenViewModel.handleActivityResult(requestCode, resultCode, data)
    }
}
