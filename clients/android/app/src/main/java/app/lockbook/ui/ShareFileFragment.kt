package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.animation.AlphaAnimation
import android.view.animation.Animation
import androidx.core.view.isEmpty
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import app.lockbook.R
import app.lockbook.databinding.FragmentShareFileBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.DetailScreen
import app.lockbook.model.StateViewModel
import app.lockbook.util.File
import app.lockbook.util.LbError
import app.lockbook.util.ShareMode
import com.google.android.material.chip.Chip
import com.google.android.material.chip.ChipGroup
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import timber.log.Timber
import java.lang.ref.WeakReference

class ShareFileFragment : Fragment() {

    private lateinit var binding: FragmentShareFileBinding
    private val activityModel: StateViewModel by activityViewModels()

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentShareFileBinding.inflate(inflater, container, false)

        val file = (activityModel.detailsScreen as DetailScreen.Share).file

        binding.materialToolbar.subtitle = file.name
        populateShares(file)

        return binding.root
    }

    private fun populateShares(file: File) {

        binding.shareFileAddUser.setOnClickListener {
            val username = binding.shareFileUsername.text.toString()
            val modeString = binding.shareFileAccessMode.text.toString()

            if(username.isEmpty()) {
                alertModel.notifyError(LbError.newUserError(getString(R.string.no_username)))
                return@setOnClickListener
            }

            if(modeString.isEmpty()) {
                alertModel.notifyError(LbError.newUserError(getString(R.string.no_access_mode)))
                return@setOnClickListener
            }

            val mode = when(modeString) {
                getString(R.string.share_mode_read) -> ShareMode.Read
                getString(R.string.share_mode_write) -> ShareMode.Write
                else -> {
                    alertModel.notifyError(LbError.newUserError(getString(R.string.basic_error)))
                    return@setOnClickListener
                }
            }


            CoreModel.shareFile(file.id, username, mode)
            alertModel.notifyWithToast(getString(R.string.please_sync))
            activityModel.launchDetailsScreen(null)
        }


        for(share in file.shares) {
            val chipGroup = when(share.mode) {
                ShareMode.Write -> binding.shareFileWriteAccessShares
                ShareMode.Read -> binding.shareFileReadAccessShares
            }

            val chip = createShareChip(chipGroup, share.sharedWith)

            chipGroup.addView(chip)
        }
    }

    private fun createShareChip(chipGroup: ChipGroup, username: String): Chip = (LayoutInflater.from(requireActivity())
        .inflate(R.layout.chip_share,binding.root) as Chip).apply {
        setOnClickListener {

            val anim = AlphaAnimation(1f,0f)
            anim.duration = 250
            anim.setAnimationListener(object : Animation.AnimationListener {
                override fun onAnimationRepeat(animation: Animation?) {}

                override fun onAnimationEnd(animation: Animation?) {
                    chipGroup.removeView(it)
                }

                override fun onAnimationStart(animation: Animation?) {}
            })

            it.startAnimation(anim)

        }

        text = username
    }

}
