package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentShareFileBinding
import app.lockbook.model.*
import com.google.android.material.chip.Chip
import net.lockbook.File
import net.lockbook.File.ShareMode
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class ShareFileFragment : Fragment() {

    private lateinit var binding: FragmentShareFileBinding
    private val activityModel: StateViewModel by activityViewModels()
    private val workspaceModel: WorkspaceViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    companion object {
        val TAG = "ShareFileFragment"
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentShareFileBinding.inflate(inflater, container, false)

        val file = (activityModel.transientScreen as TransientScreen.ShareFile).file

        activityModel.updateMainScreenUI(UpdateMainScreenUI.ToggleBottomViewNavigation)

        binding.materialToolbar.subtitle = file.getPrettyName()
        populateShares(file)

        binding.materialToolbar.setNavigationOnClickListener {
            activityModel.updateMainScreenUI(UpdateMainScreenUI.PopBackstackToWorkspace)
        }

        return binding.root
    }

    override fun onDestroy() {
        super.onDestroy()
        activityModel.updateMainScreenUI(UpdateMainScreenUI.ToggleBottomViewNavigation)
    }

    private fun populateShares(file: File) {
        binding.shareFileAddUser.setOnClickListener {
            val username = binding.shareFileUsername.text.toString()
            val modeString = binding.shareFileAccessMode.text.toString()

            if (username.isEmpty()) {
                alertModel.notifyWithToast(getString(R.string.no_username))
                return@setOnClickListener
            }

            if (modeString.isEmpty()) {
                alertModel.notifyWithToast(getString(R.string.no_access_mode))
                return@setOnClickListener
            }

            val mode = when (modeString) {
                getString(R.string.share_mode_read) -> ShareMode.Read
                getString(R.string.share_mode_write) -> ShareMode.Write
                else -> {
                    alertModel.notifyWithToast(getString(R.string.basic_error))
                    return@setOnClickListener
                }
            }

            try {
                Lb.shareFile(file.id, username, mode == ShareMode.Write)
                workspaceModel._sync.postValue(Unit)
                activityModel.updateMainScreenUI(UpdateMainScreenUI.PopBackstackToWorkspace)
            } catch (err: LbError) {
                alertModel.notifyError(err)
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
