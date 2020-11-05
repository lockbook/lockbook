package app.lockbook.ui

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import androidx.appcompat.app.AlertDialog
import androidx.fragment.app.DialogFragment
import androidx.lifecycle.ViewModelProvider
import androidx.recyclerview.widget.LinearLayoutManager
import app.lockbook.R
import app.lockbook.model.MoveFileAdapter
import app.lockbook.model.MoveFileViewModel
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

        fun newInstance(ids: Array<String>): MoveFileDialogFragment {
            val args = Bundle()
            args.putStringArray(IDS_KEY, ids)

            val fragment = MoveFileDialogFragment()
            fragment.arguments = args
            return fragment
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View? = inflater.inflate(
        R.layout.dialog_move_file,
        container,
        false
    )

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)
        val bundle = arguments
        val tempIds = bundle?.getStringArray(IDS_KEY)
        if (tempIds != null) {
            ids = tempIds
        }

        val application = requireNotNull(this.activity).application
        val moveFileViewModelFactory =
            MoveFileViewModelFactory(application.filesDir.absolutePath)
        moveFileViewModel =
            ViewModelProvider(this, moveFileViewModelFactory).get(MoveFileViewModel::class.java)
        val adapter =
            MoveFileAdapter(moveFileViewModel)

        move_file_list.layoutManager = LinearLayoutManager(context)
        move_file_list.adapter = adapter
        move_file_cancel.setOnClickListener {
            dismiss()
        }
        move_file_confirm.setOnClickListener {
            move_file_progress_bar.visibility = View.VISIBLE
            moveFileViewModel.moveFilesToFolder(ids)
        }

        moveFileViewModel.ids = ids

        moveFileViewModel.files.observe(
            viewLifecycleOwner
        ) { files ->
            adapter.files = files
        }

        moveFileViewModel.closeDialog.observe(
            viewLifecycleOwner
        ) {
            move_file_progress_bar.visibility = View.GONE
            dismiss()
        }

        moveFileViewModel.errorHasOccurred.observe(
            viewLifecycleOwner
        ) { errorText ->
            errorHasOccurred(errorText)
        }

        moveFileViewModel.unexpectedErrorHasOccurred.observe(
            viewLifecycleOwner
        ) { errorText ->
            unexpectedErrorHasOccurred(errorText)
        }

        moveFileViewModel.startInRoot()
    }

    override fun onStart() {
        super.onStart()
        dialog?.window?.setLayout(
            WindowManager.LayoutParams.MATCH_PARENT,
            WindowManager.LayoutParams.MATCH_PARENT
        )
    }

    private fun errorHasOccurred(error: String) {
        Snackbar.make(move_file_dialog, error, Snackbar.LENGTH_SHORT).show()
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
