package app.lockbook.screen

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.ImageView
import android.widget.PopupWindow
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat.finishAffinity
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.preference.*
import app.lockbook.R
import app.lockbook.model.*
import app.lockbook.ui.NumberPickerPreference
import app.lockbook.ui.NumberPickerPreferenceDialogFragment
import app.lockbook.ui.UsageBarPreference
import app.lockbook.util.getSettingsActivity
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import net.lockbook.Lb
import net.lockbook.LbError
import net.lockbook.SubscriptionInfo.AppStore
import net.lockbook.SubscriptionInfo.GooglePlay
import net.lockbook.SubscriptionInfo.Stripe
import java.lang.ref.WeakReference
import kotlin.system.exitProcess

class SettingsFragment : PreferenceFragmentCompat() {

    companion object {
        const val SCROLL_TO_PREFERENCE_KEY = "scroll_to_item_key"
        const val UPGRADE_NOW = "upgrade_now_key"
    }

    val onUpgrade =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            if (it.resultCode == SUCCESSFUL_SUBSCRIPTION_PURCHASE) {
                model.updateUsage()
            }
        }

    val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()))
    }

    val model: SettingsViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(SettingsViewModel::class.java))
                        return SettingsViewModel(requireActivity().application) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    override fun onCreatePreferences(savedInstanceState: Bundle?, rootKey: String?) {
        setPreferencesFromResource(R.xml.settings_preference, rootKey)
        setUpPreferences()
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        getSettingsActivity().scrollToPreference()?.let { preference ->
            scrollToPreference(getString(preference))
        }

        if (getSettingsActivity().upgradeNow() == true) {
            onUpgrade.launch(Intent(context, UpgradeAccountActivity::class.java))
        }

        model.sendBreadcrumb.observe(
            viewLifecycleOwner
        ) { msg ->
            alertModel.notify(msg)
        }

        model.determineSettingsInfo.observe(
            viewLifecycleOwner
        ) { settingsInfo ->
            addDataToPreferences(settingsInfo)
        }

        model.exit.observe(
            viewLifecycleOwner
        ) {
            requireActivity().finishAffinity()
            exitProcess(0)
        }

        model.notifyError.observe(
            viewLifecycleOwner
        ) { error ->
            alertModel.notifyError(error)
        }
    }

    private fun addDataToPreferences(settingsInfo: SettingsInfo) {
        val maybePaymentPlatform = settingsInfo.subscriptionInfo?.paymentPlatform

        val isPremium = settingsInfo.usage.dataCap.exact == UsageBarPreference.PAID_TIER_USAGE_BYTES
        val isOkState = (maybePaymentPlatform as? GooglePlay)?.accountState == GooglePlay.GooglePlayAccountState.Ok || (maybePaymentPlatform as? Stripe) != null || (maybePaymentPlatform as? AppStore)?.accountState == AppStore.AppStoreAccountState.Ok

        findPreference<PreferenceCategory>(getString(R.string.premium_key))!!.isVisible = isPremium
        findPreference<Preference>(getString(R.string.cancel_subscription_key))!!.isVisible = isPremium && isOkState
    }

    private fun setUpPreferences() {
        findPreference<Preference>(getString(R.string.biometric_key))?.setOnPreferenceChangeListener { _, newValue ->
            if (newValue is String) {
                BiometricModel.verify(
                    requireActivity(),
                    VerificationItem.BiometricsSettingsChange,
                    {
                        findPreference<ListPreference>(getString(R.string.biometric_key))?.value = newValue
                    }
                )
            }

            false
        }

        findPreference<Preference>(getString(R.string.background_sync_period_key))?.isEnabled =
            PreferenceManager.getDefaultSharedPreferences(
                requireContext()
            ).getBoolean(
                getString(R.string.background_sync_enabled_key),
                true
            )

        if (!BiometricModel.isBiometricVerificationAvailable(requireContext())) {
            findPreference<ListPreference>(getString(R.string.biometric_key))?.isEnabled = false
        }
    }

    override fun onDisplayPreferenceDialog(preference: Preference) {
        if (preference is NumberPickerPreference) {
            val numberPickerPreferenceDialog =
                NumberPickerPreferenceDialogFragment.newInstance(preference.key)
            @Suppress("DEPRECATION")
            numberPickerPreferenceDialog.setTargetFragment(this, 0)
            numberPickerPreferenceDialog.show(parentFragmentManager, null)
        } else {
            super.onDisplayPreferenceDialog(preference)
        }
    }

    override fun onPreferenceTreeClick(preference: Preference): Boolean {
        when (preference.key) {
            getString(R.string.export_account_qr_key) -> BiometricModel.verify(
                requireActivity(),
                VerificationItem.ViewPrivateKey,
                ::exportAccountQR
            )
            getString(R.string.export_account_raw_key) -> BiometricModel.verify(
                requireActivity(),
                VerificationItem.ViewPrivateKey,
                ::exportAccountRaw
            )
            getString(R.string.export_account_phrase_key) -> BiometricModel.verify(
                requireActivity(),
                VerificationItem.ViewPrivateKey,
                ::exportAccountPhrase
            )
            getString(R.string.debug_info_key) -> startActivity(Intent(context, DebugInfoActivity::class.java))
            getString(R.string.background_sync_enabled_key) ->
                findPreference<Preference>(getString(R.string.background_sync_period_key))?.isEnabled =
                    (preference as SwitchPreference).isChecked
            getString(R.string.cancel_subscription_key) -> {
                val dialog = MaterialAlertDialogBuilder(requireContext())
                    .setTitle(R.string.settings_cancel_sub_confirmation_title)
                    .setMessage(R.string.settings_cancel_sub_confirmation_details)
                    .setPositiveButton(R.string.yes) { _, _ ->
                        model.cancelSubscription()
                    }
                    .setNegativeButton(R.string.no, null)

                dialog.show()
            }
            getString(R.string.logout_key) -> {
                val dialog = MaterialAlertDialogBuilder(requireContext())
                    .setTitle(R.string.logout)
                    .setMessage(R.string.logout_confirmation_details)
                    .setPositiveButton(R.string.yes) { _, _ ->
                        model.logout()
                    }
                    .setNegativeButton(R.string.no, null)

                dialog.show()
            }
            getString(R.string.delete_account_key) -> {
                val dialog = MaterialAlertDialogBuilder(requireContext())
                    .setTitle(R.string.delete_account)
                    .setMessage(R.string.delete_account_confirmation_details)
                    .setPositiveButton(R.string.yes) { _, _ ->
                        model.deleteAccount()
                    }
                    .setNegativeButton(R.string.no, null)

                dialog.show()
            }
            else -> super.onPreferenceTreeClick(preference)
        }

        return true
    }

    private fun exportAccountQR() {
        try {
            val bitmap = BarcodeEncoder().encodeBitmap(
                Lb.exportAccountPrivateKey(),
                BarcodeFormat.QR_CODE,
                400,
                400
            )

            val qrCodeView = layoutInflater.inflate(
                R.layout.popup_window_qr_code,
                view as ViewGroup,
                false
            )
            qrCodeView.findViewById<ImageView>(R.id.qr_code).setImageBitmap(bitmap)
            val popUpWindow = PopupWindow(qrCodeView, 900, 900, true)
            popUpWindow.showAtLocation(view, Gravity.CENTER, 0, 0)
        } catch (err: LbError) {
            alertModel.notifyError(err)
        }
    }

    private fun exportAccountRaw() {
        try {
            val clipBoard: ClipboardManager =
                requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clipBoardData = ClipData.newPlainText("account string", Lb.exportAccountPrivateKey())
            clipBoard.setPrimaryClip(clipBoardData)
            alertModel.notify(getString(R.string.settings_export_account_copied))
        } catch (err: LbError) {
            alertModel.notifyError(err)
        }
    }
    private fun exportAccountPhrase() {
        try {
            val clipBoard: ClipboardManager =
                requireContext().getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clipBoardData = ClipData.newPlainText("account phrase", Lb.exportAccountPhrase())
            clipBoard.setPrimaryClip(clipBoardData)
            alertModel.notify(getString(R.string.settings_export_account_phrase_copied))
        } catch (err: LbError) {
            alertModel.notifyError(err)
        }
    }
}
