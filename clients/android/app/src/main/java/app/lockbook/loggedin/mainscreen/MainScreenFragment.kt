package app.lockbook.loggedin.mainscreen

import android.content.Intent
import android.os.Bundle
import android.util.Log
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
import app.lockbook.loggedin.listfiles.ListFilesAdapter
import app.lockbook.loggedin.popupinfo.PopUpInfoActivity
import app.lockbook.loggedin.texteditor.TextEditorActivity
import app.lockbook.utils.FileMetadata
import kotlinx.coroutines.withContext

class MainScreenFragment : Fragment() {

    companion object {
        const val NEW_FILE_REQUEST_CODE: Int = 1
        const val TEXT_EDITOR_REQUEST_CODE: Int = 2
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
            MainScreenViewModelFactory(
                application.filesDir.absolutePath
            )
        mainScreenViewModel =
            ViewModelProvider(this, mainScreenViewModelFactory).get(MainScreenViewModel::class.java)
        val adapter = ListFilesAdapter(mainScreenViewModel)

        binding.mainScreenViewModel = mainScreenViewModel
        binding.filesFolders.adapter = adapter
        binding.filesFolders.layoutManager = LinearLayoutManager(context)
        binding.lifecycleOwner = this

        mainScreenViewModel.filesFolders.observe(viewLifecycleOwner, Observer {
            updateRecyclerView(it, adapter)
        })

        mainScreenViewModel.navigateToFileEditor.observe(viewLifecycleOwner, Observer {
            Log.i("SmailBarkouch", it)
            navigateToFileEditor(it)
        })

        mainScreenViewModel.navigateToPopUpInfo.observe(viewLifecycleOwner, Observer {
            navigateToPopUpInfo(it)
        })

        mainScreenViewModel.navigateToNewFileFolder.observe(viewLifecycleOwner, Observer {
            navigateToNewFileFolder(it)
        })

        mainScreenViewModel.startListFilesFolders()

        return binding.root
    }

    private fun updateRecyclerView(
        it: List<FileMetadata>,
        adapter: ListFilesAdapter
    ) {
        it?.let {
            if (it.isEmpty()) {
                adapter.filesFolders = listOf()
            } else {
                adapter.filesFolders = it
            }
        }
    }

    private fun navigateToFileEditor(it: String) {
        it?.let {
            val intent = Intent(context, TextEditorActivity::class.java)
            intent.putExtra("text", it)
            startActivityForResult(intent, TEXT_EDITOR_REQUEST_CODE)
        }
    }

    private fun navigateToPopUpInfo(it: FileMetadata) {
        it?.let {
            val intent = Intent(context, PopUpInfoActivity::class.java)
            intent.putExtra("name", it.name)
            intent.putExtra("id", it.id)
            intent.putExtra("fileType", it.file_type.toString())
            intent.putExtra("metadataVersion", it.metadata_version.toString())
            intent.putExtra("contentVersion", it.content_version.toString())
            startActivity(intent)
        }
    }


    private fun navigateToNewFileFolder(it: Boolean) {
        it?.let {
            if (it) {
                val intent = Intent(context, NewFileFolderActivity::class.java)
                intent.putExtra(
                    "parentUuid",
                    mainScreenViewModel.fileFolderModel.parentFileMetadata.id
                )
                intent.putExtra("path", mainScreenViewModel.path)
                startActivityForResult(intent, NEW_FILE_REQUEST_CODE)
            }
        }
    }

    fun onBackPressed(): Boolean {
        return mainScreenViewModel.quitOrNot()
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        when(requestCode) {
            NEW_FILE_REQUEST_CODE -> {
                mainScreenViewModel.refreshFilesFolderList()
            }
            TEXT_EDITOR_REQUEST_CODE -> {
                data?.let {
                    if(resultCode == TextEditorActivity.OK) {
                        Log.i("SmailBarkouch", "SUCCESS, but why no save?")
                        mainScreenViewModel.writeNewTextToDocument(data.getStringExtra("text"))
                    } else {
                        Toast.makeText(context, "TEXT DID NOT SAVE", Toast.LENGTH_LONG).show()
                    }
                }
            }
        }
    }
}