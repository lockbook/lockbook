package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.appcompat.app.AlertDialog
import androidx.databinding.DataBindingUtil
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.model.*
import app.lockbook.modelfactory.MoveFileViewModelFactory
import app.lockbook.util.Messages
import com.google.android.material.snackbar.Snackbar
import kotlinx.android.synthetic.main.dialog_move_file.*

class MoveFileDialogFragment : DialogFragment() {

    lateinit var ids: Array<String>
    lateinit var moveFileViewModel: MoveFileViewModel

    companion object {

        const val MOVE_FILE_DIALOG_TAG = "MoveFileDialogFragment"

        private const val IDS_KEY = "IDS_KEY"

        fun newInstance(ids: List<String>): MoveFileDialogFragment {
            val args = Bundle()
            args.putStringArray(IDS_KEY, ids.toTypedArray())

            val fragment = MoveFileDialogFragment()
            fragment.arguments = args
            return fragment
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? {
        val binding: MoveFileDialogFragment = DataBindingUtil.inflate(
            inflater,
            R.layout.dialog_move_file,
            container,
            false
        )

        val application = requireNotNull(this.activity).application
        val moveFileViewModelFactory =
            MoveFileViewModelFactory(application.filesDir.absolutePath, application)
        moveFileViewModel =
            ViewModelProvider(this, moveFileViewModelFactory).get(MoveFileViewModel::class.java)
        val adapter =
            MoveFileAdapter(moveFileViewModel)

        binding.moveFileViewModel = moveFileViewModel
        binding.move_file_list.layoutManager = LinearLayoutManager(context)
        binding.move_file_list.adapter = adapter
        binding.move_file_cancel.setOnClickListener {
            dismiss()
        }
        binding.move_file_confirm.setOnClickListener {
            moveFileViewModel.moveFilesToFolder(ids)
        }

        moveFileViewModel.files.observe(
            viewLifecycleOwner
        ) { files ->
            adapter.files = files
        }

        moveFileViewModel.errorHasOccurred.observe(
            viewLifecycleOwner
        ) { errorText ->
            errorHasOccurred(container, errorText)
        }

        moveFileViewModel.unexpectedErrorHasOccurred.observe(
            viewLifecycleOwner
        ) { errorText ->
            unexpectedErrorHasOccurred(errorText)
        }

        return view
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        val bundle = arguments
        val tempIds = bundle?.getStringArray(IDS_KEY)
        if (tempIds != null) {
            ids = tempIds
        }
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.MATCH_PARENT
        )
    }

    private fun errorHasOccurred(view: ViewGroup?, error: String) {
        if (view != null) {
            Snackbar.make(view, error, Snackbar.LENGTH_SHORT).show()
        } else {
            Snackbar.make(move_file_dialog, error, Snackbar.LENGTH_SHORT).show()
        }
    }

    private fun unexpectedErrorHasOccurred(error: String) {
        AlertDialog.Builder(requireContext(), R.style.DarkBlue_Dialog)
            .setTitle(Messages.UNEXPECTED_ERROR)
            .setMessage(error)
            .setOnCancelListener {
                dismiss()
            }
            .show()
    }
}
