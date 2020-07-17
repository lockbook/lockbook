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
import app.lockbook.loggedin.newfilefolder.NewFileFolderActivity
import app.lockbook.loggedin.listfiles.FilesFoldersAdapter
import app.lockbook.loggedin.popupinfo.PopUpInfoActivity
import app.lockbook.loggedin.texteditor.TextEditorActivity
import app.lockbook.utils.FileMetadata

class MainScreenFragment : Fragment() {

    companion object {
        const val NEW_FILE_REQUEST_CODE: Int = 101
        const val TEXT_EDITOR_REQUEST_CODE: Int = 102
        const val POP_UP_INFO_REQUEST_CODE: Int = 103
    }

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
        val adapter = FilesFoldersAdapter(mainScreenViewModel)

        binding.mainScreenViewModel = mainScreenViewModel
        binding.filesFolders.adapter = adapter
        binding.filesFolders.layoutManager = LinearLayoutManager(context)
        binding.lifecycleOwner = this

        mainScreenViewModel.filesFolders.observe(viewLifecycleOwner, Observer { filesFolders ->
            updateRecyclerView(filesFolders, adapter)
        })

        mainScreenViewModel.navigateToFileEditor.observe(
            viewLifecycleOwner,
            Observer { fileContents ->
                navigateToFileEditor(fileContents)
            })

        mainScreenViewModel.navigateToPopUpInfo.observe(
            viewLifecycleOwner,
            Observer { fileMetadata ->
                navigateToPopUpInfo(fileMetadata)
            })

        mainScreenViewModel.navigateToNewFileFolder.observe(
            viewLifecycleOwner,
            Observer { newFile ->
                navigateToNewFileFolder(newFile)
            })

        mainScreenViewModel.errorHasOccurred.observe(viewLifecycleOwner, Observer { errorText ->
            errorHasOccurred(errorText)
        })

        mainScreenViewModel.startListFilesFolders()

        return binding.root
    }

    private fun updateRecyclerView(
        filesFolders: List<FileMetadata>,
        adapter: FilesFoldersAdapter
    ) {
        if (filesFolders.isEmpty()) {
            adapter.filesFolders = listOf()
        } else {
            adapter.filesFolders = filesFolders
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


    private fun navigateToNewFileFolder(newFile: Boolean) {
        if (newFile) {
            val intent = Intent(context, NewFileFolderActivity::class.java)
            startActivityForResult(intent, NEW_FILE_REQUEST_CODE)
        }
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