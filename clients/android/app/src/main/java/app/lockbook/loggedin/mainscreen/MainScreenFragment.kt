package app.lockbook.loggedin.mainscreen

import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
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
            it?.let {
                if (it.isEmpty()) {
                    adapter.filesFolders = listOf()
                } else {
                    adapter.filesFolders = it
                }
            }
        })

        mainScreenViewModel.navigateToFileEditor.observe(viewLifecycleOwner, Observer {

        })

        mainScreenViewModel.navigateToPopUpInfo.observe(viewLifecycleOwner, Observer {
            it?.let {
                val intent = Intent(context, PopUpInfoActivity::class.java)
                intent.putExtra("name", it.name)
                intent.putExtra("id", it.id)
                intent.putExtra("fileType", it.file_type.toString())
                intent.putExtra("metadataVersion", it.metadata_version.toString())
                intent.putExtra("contentVersion", it.content_version.toString())
                startActivity(intent)
            }
        })

        mainScreenViewModel.navigateToNewFileFolder.observe(viewLifecycleOwner, Observer {
            it?.let {
                if (it) {
                    val intent = Intent(context, NewFileFolderActivity::class.java)
                    intent.putExtra(
                        "parentUuid",
                        mainScreenViewModel.fileFolderModel.parentFileMetadata.id
                    )
                    intent.putExtra("path", application.filesDir.absolutePath)
                    startActivity(intent)
                }
            }
        })

        mainScreenViewModel.startListFilesFolders()

        return binding.root
    }

    fun onBackPressed(): Boolean {
        if (mainScreenViewModel.fileFolderModel.parentFileMetadata.id
            == mainScreenViewModel.fileFolderModel.parentFileMetadata.parent) {
            return false
        }

        mainScreenViewModel.upADirectory()

        return true
    }

    override fun onResume() {
        super.onResume()

    }
}