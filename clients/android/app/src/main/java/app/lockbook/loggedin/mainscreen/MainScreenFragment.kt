package app.lockbook.loggedin.mainscreen

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
import app.lockbook.databinding.FragmentListFilesBinding
import app.lockbook.loggedin.listfiles.ListFilesAdapter

class MainScreenFragment: Fragment() {
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val binding: FragmentListFilesBinding = DataBindingUtil.inflate(
            inflater, R.layout.fragment_list_files, container, false
        )

        val application = requireNotNull(this.activity).application
        val mainScreenViewModelFactory =
            MainScreenViewModelFactory(
                application.filesDir.absolutePath
            )
        val mainScreenViewModel: MainScreenViewModel = ViewModelProvider(this, mainScreenViewModelFactory).get(
            MainScreenViewModel::class.java)
        val adapter = ListFilesAdapter(mainScreenViewModel)


        binding.mainScreenViewModel = mainScreenViewModel
        binding.filesFolders.adapter = adapter
        binding.lifecycleOwner = this
        binding.filesFolders.layoutManager = LinearLayoutManager(context)

        mainScreenViewModel.filesFolders.observe(viewLifecycleOwner, Observer {
            if(it.isEmpty()) {
                adapter.filesFolders = listOf()
            } else {
                adapter.filesFolders = it
            }
        })

        mainScreenViewModel.navigateToFileEditor.observe(viewLifecycleOwner, Observer {
            Toast.makeText(context, "Yo wanted to open the document? Not implemented yet, sorry buddy.", Toast.LENGTH_LONG)
        })

        mainScreenViewModel.navigateToPopUpInfo.observe(viewLifecycleOwner, Observer {

        })

        mainScreenViewModel.getRootMetadata()

        return binding.root
    }
}