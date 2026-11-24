package app.lockbook.screen

import android.os.Bundle
import android.view.View
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import androidx.preference.Preference
import androidx.preference.PreferenceCategory
import androidx.preference.PreferenceFragmentCompat
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.BiometricModel
import app.lockbook.model.SubAccountsViewModel
import app.lockbook.model.VerificationItem
import app.lockbook.util.copyToClipboard
import app.lockbook.util.showAccountKeyQR
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import kotlinx.coroutines.launch
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class SubAccountFragment : PreferenceFragmentCompat() {
    private val model: SubAccountsViewModel by activityViewModels()
    private val username by lazy { requireArguments().getString(USERNAME_KEY)!! }
    private val alertModel by lazy { AlertModel(WeakReference(requireActivity())) }
    private lateinit var storage: Preference
    private val actions = mutableListOf<Preference>()

    companion object {
        private const val USERNAME_KEY = "username"

        fun newInstance(username: String): SubAccountFragment =
            SubAccountFragment().apply {
                arguments = Bundle().apply { putString(USERNAME_KEY, username) }
            }
    }

    override fun onCreatePreferences(
        savedInstanceState: Bundle?,
        rootKey: String?,
    ) {
        val context = preferenceManager.context
        val screen = preferenceManager.createPreferenceScreen(context)
        actions.clear()

        storage = info(R.string.sub_account_storage)
        screen.addPreference(info(R.string.username).apply { summary = username })
        screen.addPreference(storage)

        val keys =
            PreferenceCategory(context).apply {
                title = getString(R.string.sub_account_keys)
                isIconSpaceReserved = false
            }
        screen.addPreference(keys)
        keys.addPreference(
            action(R.string.settings_export_account_raw) {
                reveal(model::exportKey) { key ->
                    copyToClipboard("account string", key)
                    alertModel.notify(getString(R.string.settings_export_account_copied))
                }
            },
        )
        keys.addPreference(
            action(R.string.settings_export_account_phrase) {
                reveal(model::exportPhrase) { phrase ->
                    copyToClipboard("account phrase", phrase)
                    alertModel.notify(getString(R.string.settings_export_account_phrase_copied))
                }
            },
        )
        keys.addPreference(action(R.string.sub_account_export_qr) { reveal(model::exportKey, this::showAccountKeyQR) })
        keys.addPreference(
            Preference(context).apply {
                summary = getString(R.string.sub_account_keys_description)
                isSelectable = false
                isIconSpaceReserved = false
            },
        )

        val delete =
            PreferenceCategory(context).apply {
                title = getString(R.string.delete_file_delete)
                isIconSpaceReserved = false
            }
        screen.addPreference(delete)
        delete.addPreference(action(R.string.sub_account_delete, ::confirmDelete))

        preferenceScreen = screen
    }

    override fun onViewCreated(
        view: View,
        savedInstanceState: Bundle?,
    ) {
        super.onViewCreated(view, savedInstanceState)
        model.state.observe(viewLifecycleOwner) { state ->
            state.subAccounts.find { it.username == username }?.let { storage.summary = Lb.bytesToHuman(it.usage) }
            actions.forEach { it.isEnabled = !state.working }
        }
    }

    override fun onStart() {
        super.onStart()
        (requireActivity() as SubAccountsActivity).setToolbarTitle(username)
    }

    private fun info(title: Int): Preference =
        Preference(preferenceManager.context).apply {
            setTitle(title)
            isSelectable = false
            isIconSpaceReserved = false
        }

    private fun action(
        title: Int,
        onClick: () -> Unit,
    ): Preference =
        Preference(preferenceManager.context)
            .apply {
                setTitle(title)
                isIconSpaceReserved = false
                setOnPreferenceClickListener {
                    onClick()
                    true
                }
            }.also(actions::add)

    private fun reveal(
        export: suspend (String) -> String,
        show: (String) -> Unit,
    ) {
        BiometricModel.verify(
            requireActivity(),
            VerificationItem.ViewPrivateKey,
            {
                viewLifecycleOwner.lifecycleScope.launch {
                    try {
                        show(export(username))
                    } catch (err: LbError) {
                        alertModel.notifyError(err)
                    }
                }
            },
        )
    }

    private fun confirmDelete() {
        MaterialAlertDialogBuilder(requireContext())
            .setTitle(getString(R.string.sub_account_delete_title, username))
            .setMessage(R.string.sub_account_delete_details)
            .setPositiveButton(R.string.delete_file_delete) { _, _ -> delete() }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    private fun delete() {
        viewLifecycleOwner.lifecycleScope.launch {
            try {
                model.delete(username)
                parentFragmentManager.popBackStack()
            } catch (err: LbError) {
                alertModel.notifyError(err)
            }
        }
    }
}
