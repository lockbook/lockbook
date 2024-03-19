package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentShareFileBinding
import app.lockbook.model.*
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.chip.Chip
import java.lang.ref.WeakReference

class ShareFileFragment : Fragment() {

    private lateinit var binding: FragmentShareFileBinding
    private val activityModel: StateViewModel by activityViewModels()
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    companion object {
        val admittedUsernames = listOf("parth", "smail", "travis", "adam", "steve", "krishma")
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentShareFileBinding.inflate(inflater, container, false)

//        val file = (activityModel.detailScreen as DetailScreen.Share).file

//        binding.materialToolbar.subtitle = file.name
//        populateShares(file)
//
//        binding.materialToolbar.setNavigationOnClickListener {
//            activityModel.launchDetailScreen(null)
//        }

        when (val getAccountResult = CoreModel.getAccount()) {
            is Ok ->
                if (admittedUsernames.any { username ->
                    username == getAccountResult.value.username
                }
                ) {
                    binding.shareFileAddUser.visibility = View.VISIBLE
                }
            is Err -> alertModel.notifyError(getAccountResult.error.toLbError(requireContext().resources))
        }

        return binding.root
    }

    private fun populateShares(file: File) {
        binding.shareFileAddUser.setOnClickListener {
            val username = binding.shareFileUsername.text.toString()
            val modeString = binding.shareFileAccessMode.text.toString()

            if (username.isEmpty()) {
                alertModel.notifyError(LbError.newUserError(getString(R.string.no_username)))
                return@setOnClickListener
            }

            if (modeString.isEmpty()) {
                alertModel.notifyError(LbError.newUserError(getString(R.string.no_access_mode)))
                return@setOnClickListener
            }

            val mode = when (modeString) {
                getString(R.string.share_mode_read) -> ShareMode.Read
                getString(R.string.share_mode_write) -> ShareMode.Write
                else -> {
                    alertModel.notifyError(LbError.newUserError(getString(R.string.basic_error)))
                    return@setOnClickListener
                }
            }

            when (val result = CoreModel.shareFile(file.id, username, mode)) {
                is Ok -> {
                    workspaceModel._sync.postValue(Unit)
//                    activityModel.launchDetailScreen(null)
                }
                is Err -> alertModel.notifyError(result.error.toLbError(resources))
            }
        }

        for (share in file.shares) {
            val chipGroup = when (share.mode) {
                ShareMode.Write -> binding.shareFileWriteAccessShares
                ShareMode.Read -> binding.shareFileReadAccessShares
            }

            val chip = createShareChip(share.sharedWith)

            chipGroup.addView(chip)
        }
    }

    private fun createShareChip(username: String): Chip = (
        LayoutInflater.from(requireActivity())
            .inflate(R.layout.chip_share, null) as Chip
        )
        .apply {
            text = username
        }
}
