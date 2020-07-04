package app.lockbook.listfiles

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.Fragment
import androidx.lifecycle.Observer
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.databinding.FragmentListFilesBinding

class ListFilesFragment: Fragment() {
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {

        val binding: FragmentListFilesBinding = DataBindingUtil.inflate(
            inflater, R.layout.fragment_list_files, container, false
        )

        val application = requireNotNull(this.activity).application

        val listFilesViewModelFactory = ListFilesViewModelFactory(application.filesDir)
        val listFilesViewModel: ListFilesViewModel = ViewModelProvider(this, listFilesViewModelFactory).get(ListFilesViewModel::class.java)

        binding.listFilesViewModel = listFilesViewModel
        val adapter = ListFilesAdapter()
        binding.filesFolders.adapter = adapter

        binding.lifecycleOwner = this

        binding.filesFolders.layoutManager = LinearLayoutManager(context)

        listFilesViewModel.filesFolders.observe(viewLifecycleOwner, Observer {
            if(it.isEmpty()) {
                adapter.filesFolders = listOf()
            } else {
                adapter.filesFolders = it
            }
        })

        return binding.root
    }
}