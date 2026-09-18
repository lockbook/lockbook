@file:Suppress("ktlint:standard:backing-property-naming")

package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import app.lockbook.databinding.SheetPhotoSourceBinding
import com.google.android.material.bottomsheet.BottomSheetBehavior
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.bottomsheet.BottomSheetDialogFragment

/** The editor's photo-source choice; the system picker handles the actual album browsing. */
class PhotoSourceBottomSheetFragment : BottomSheetDialogFragment() {
    private var _binding: SheetPhotoSourceBinding? = null
    private val binding get() = _binding!!

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = SheetPhotoSourceBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)
        binding.takePhoto.setOnClickListener { select(SOURCE_CAMERA) }
        binding.choosePhoto.setOnClickListener { select(SOURCE_LIBRARY) }
    }

    override fun onStart() {
        super.onStart()
        (dialog as? BottomSheetDialog)?.behavior?.apply {
            state = BottomSheetBehavior.STATE_EXPANDED
            skipCollapsed = true
        }
    }

    override fun onDestroyView() {
        _binding = null
        super.onDestroyView()
    }

    private fun select(source: String) {
        binding.takePhoto.isEnabled = false
        binding.choosePhoto.isEnabled = false
        parentFragmentManager.setFragmentResult(
            REQUEST_KEY,
            Bundle().apply { putString(SOURCE_KEY, source) },
        )
        dismiss()
    }

    companion object {
        const val TAG = "PhotoSourceBottomSheetFragment"
        const val REQUEST_KEY = "photo_source"
        const val SOURCE_KEY = "source"
        const val SOURCE_CAMERA = "camera"
        const val SOURCE_LIBRARY = "library"
    }
}
