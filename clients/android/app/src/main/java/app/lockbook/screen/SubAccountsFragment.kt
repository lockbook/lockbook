package app.lockbook.screen

import android.content.DialogInterface
import android.os.Bundle
import android.view.View
import android.view.inputmethod.EditorInfo
import androidx.appcompat.app.AlertDialog
import androidx.core.widget.doAfterTextChanged
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import androidx.preference.Preference
import androidx.preference.PreferenceFragmentCompat
import app.lockbook.R
import app.lockbook.databinding.DialogNewSubAccountBinding
import app.lockbook.model.SUB_ACCOUNT_SEPARATOR
import app.lockbook.model.SubAccountsState
import app.lockbook.model.SubAccountsViewModel
import app.lockbook.util.inlineMessage
import app.lockbook.util.requestKeyboardFocus
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError

class SubAccountsFragment : PreferenceFragmentCompat() {
    private val model: SubAccountsViewModel by activityViewModels()
    private var newDialog: AlertDialog? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        model.load()
    }

    override fun onCreatePreferences(
        savedInstanceState: Bundle?,
        rootKey: String?,
    ) {
        preferenceScreen = preferenceManager.createPreferenceScreen(preferenceManager.context)
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)
        model.state.observe(viewLifecycleOwner, ::render)
    }

    override fun onStart() {
        super.onStart()
        (requireActivity() as SubAccountsActivity).setToolbarTitle(getString(R.string.sub_accounts))
    }

    override fun onDestroyView() {
        newDialog?.dismiss()
        newDialog = null
        super.onDestroyView()
    }

    private fun render(state: SubAccountsState) {
        val context = preferenceManager.context
        preferenceScreen.removeAll()

        state.subAccounts.forEach { subAccount ->
            preferenceScreen.addPreference(
                Preference(context).apply {
                    title = subAccount.username
                    summary = Lb.bytesToHuman(subAccount.usage)
                    setIcon(R.drawable.ic_baseline_key_24)
                    setOnPreferenceClickListener {
                        open(subAccount.username)
                        true
                    }
                },
            )
        }

        val status =
            when {
                !state.loaded -> state.loadError?.inlineMessage(context) ?: getString(R.string.sub_accounts_loading)
                state.subAccounts.isEmpty() -> getString(R.string.sub_accounts_empty)
                else -> null
            }
        if (status != null) preferenceScreen.addPreference(note(status))

        preferenceScreen.addPreference(note(getString(R.string.sub_accounts_description)))
        preferenceScreen.addPreference(
            Preference(context).apply {
                title = getString(R.string.sub_account_new)
                if (state.isPremium == false) summary = getString(R.string.sub_account_new_needs_premium)
                setIcon(R.drawable.ic_outline_person_add_alt_24)
                isEnabled = state.isPremium == true && !state.working
                setOnPreferenceClickListener {
                    showNewDialog()
                    true
                }
            },
        )
    }

    private fun note(text: String): Preference =
        Preference(preferenceManager.context).apply {
            summary = text
            isSelectable = false
        }

    private fun open(username: String) {
        parentFragmentManager
            .beginTransaction()
            .replace(R.id.settings_preference_layout, SubAccountFragment.newInstance(username))
            .addToBackStack(null)
            .commit()
    }

    private fun showNewDialog() {
        val binding = DialogNewSubAccountBinding.inflate(layoutInflater)
        binding.newSubAccountHolder.prefixText = model.parent + SUB_ACCOUNT_SEPARATOR

        val dialog =
            MaterialAlertDialogBuilder(requireContext())
                .setTitle(R.string.sub_account_new)
                .setMessage(getString(R.string.sub_account_new_details, model.parent))
                .setView(binding.root)
                .setNegativeButton(R.string.cancel, null)
                .setPositiveButton(R.string.create, null)
                .create()

        fun create() {
            val button = dialog.getButton(DialogInterface.BUTTON_POSITIVE)
            val label =
                binding.newSubAccount.text
                    ?.toString()
                    ?.trim()
                    .orEmpty()
            if (label.isEmpty() || !button.isEnabled) return

            button.isEnabled = false
            binding.newSubAccountHolder.error = null
            viewLifecycleOwner.lifecycleScope.launch {
                try {
                    model.create(label)
                    dialog.dismiss()
                } catch (err: LbError) {
                    binding.newSubAccountHolder.error = err.inlineMessage(requireContext())
                    button.isEnabled = true
                }
            }
        }

        binding.newSubAccount.doAfterTextChanged { binding.newSubAccountHolder.error = null }
        binding.newSubAccount.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_DONE) create()
            true
        }
        dialog.setOnShowListener {
            dialog.getButton(DialogInterface.BUTTON_POSITIVE).setOnClickListener { create() }
            dialog.window.requestKeyboardFocus(binding.newSubAccount)
        }
        dialog.show()
        newDialog = dialog
    }
}
