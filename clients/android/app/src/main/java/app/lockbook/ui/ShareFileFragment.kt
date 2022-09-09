package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.core.view.isEmpty
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentShareFileBinding
import app.lockbook.model.DetailScreen
import app.lockbook.model.StateViewModel
import app.lockbook.util.File
import app.lockbook.util.ShareMode
import com.google.android.material.chip.Chip

class ShareFileFragment : Fragment() {

    private lateinit var binding: FragmentShareFileBinding
    private val activityModel: StateViewModel by activityViewModels()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentShareFileBinding.inflate(inflater, container, false)

        val file = (activityModel.detailsScreen as DetailScreen.Share).file

        populateShares(file)

        return binding.root
    }

    private fun populateShares(file: File) {

        binding.shareFileAddUser.setOnClickListener {
            if(binding.shareFileNew.isEmpty()) {

            }

            if(binding.shareFileAccessMode.text.isEmpty()) {

            }
        }


        for(share in file.shares) {
            val chip = createShareChip(share.sharedWith)

            when(share.mode) {
                ShareMode.Write -> binding.shareFileWriteAccessShares.addView(chip)
                ShareMode.Read -> binding.shareFileReadAccessShares.addView(chip)
            }
        }
    }

    private fun createShareChip(username: String): Chip = (LayoutInflater.from(requireActivity())
        .inflate(R.layout.chip_share,binding.root) as Chip).apply {
        text = username
    }

}
