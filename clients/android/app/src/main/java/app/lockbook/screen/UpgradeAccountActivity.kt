package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.os.Bundle
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityUpgradeAccountBinding
import app.lockbook.model.AlertModel
import com.google.android.material.card.MaterialCardView
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class UpgradeAccountActivity : AppCompatActivity() {

    private lateinit var binding: ActivityUpgradeAccountBinding

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    enum class AccountTier {
        Free,
        PremiumMonthly,
    }

    private var selectedTier = AccountTier.Free

    private fun screenIsLarge(): Boolean {
        val screenSize = resources.configuration.screenLayout and Configuration.SCREENLAYOUT_SIZE_MASK

        return screenSize == Configuration.SCREENLAYOUT_SIZE_LARGE ||
            screenSize == Configuration.SCREENLAYOUT_SIZE_XLARGE
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityUpgradeAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        @SuppressLint("SourceLockedOrientationActivity")
        if (!screenIsLarge()) {
            requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        }

        if (savedInstanceState != null) {
            selectedTier = AccountTier.valueOf(savedInstanceState.getString(SELECTED_TIER_KEY, AccountTier.Free.name))
        }

        binding.upgradeAccountTierFree.setOnClickListener(clickListener)
        binding.upgradeAccountTierPremiumMonthly.setOnClickListener(clickListener)

        binding.exitBilling.setOnClickListener {
            finish()
        }

        (application as App).billingClientLifecycle.billingEvent.observe(this) { billingEvent ->
            handleBillingEvent(billingEvent)
        }

        val selectedTierCardView = when (selectedTier) {
            AccountTier.Free -> binding.upgradeAccountTierFree
            AccountTier.PremiumMonthly -> binding.upgradeAccountTierPremiumMonthly
        }

        selectedTierCardView.isChecked = true
        binding.subscribeToPlan.isEnabled = selectedTier != AccountTier.Free
        binding.subscribeToPlan.setOnClickListener {
            launchPurchaseFlow(selectedTier)
        }
    }

    private fun handleBillingEvent(billingEvent: BillingEvent) {
        when (billingEvent) {
            BillingEvent.NotifyUnrecoverableError -> alertModel.notify(resources.getString(R.string.unrecoverable_billing_error)) {
                finish()
            }
            is BillingEvent.SuccessfulPurchase -> {
                uiScope.launch {
                    binding.progressOverlay.visibility = View.VISIBLE
                    binding.subscribeToPlan.isEnabled = false

                    withContext(Dispatchers.IO) {
                        try {
                            Lb.upgradeAccountGooglePlay(billingEvent.purchaseToken, billingEvent.accountId)
                            binding.progressOverlay.visibility = View.GONE
                            binding.subscribeToPlan.isEnabled = true

                            alertModel.notifySuccessfulPurchaseConfirm {
                                setResult(SUCCESSFUL_SUBSCRIPTION_PURCHASE)
                                this@UpgradeAccountActivity.finish()
                            }
                        } catch (err: LbError) {
                            alertModel.notifyError(err)
                        }
                    }
                }
            }
            is BillingEvent.NotifyError -> alertModel.notifyError(billingEvent.error)
            is BillingEvent.NotifyErrorMsg -> alertModel.notifyWithToast(billingEvent.error)
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putString(SELECTED_TIER_KEY, selectedTier.name)

        super.onSaveInstanceState(outState)
    }

    private fun toggleSubscribeButton() {
        binding.subscribeToPlan.isEnabled = selectedTier != AccountTier.Free
    }

    private val clickListener = View.OnClickListener { tierCardView ->
        val oldSelectedTier = selectedTier

        selectedTier = when (tierCardView) {
            binding.upgradeAccountTierFree -> AccountTier.Free
            binding.upgradeAccountTierPremiumMonthly -> AccountTier.PremiumMonthly
            else -> AccountTier.Free
        }

        val oldTierCardView = when (oldSelectedTier) {
            AccountTier.Free -> binding.upgradeAccountTierFree
            AccountTier.PremiumMonthly -> binding.upgradeAccountTierPremiumMonthly
        }

        toggleSubscribeButton()
        oldTierCardView.isChecked = false
        (tierCardView as MaterialCardView).isChecked = true
    }

    private fun launchPurchaseFlow(selectedTier: AccountTier) {
        if (selectedTier == AccountTier.PremiumMonthly) {
            (application as App).billingClientLifecycle.launchBillingFlow(this, selectedTier)
        }
    }

    companion object {
        const val SELECTED_TIER_KEY = "selected_tier_key"
    }
}

const val SUCCESSFUL_SUBSCRIPTION_PURCHASE = 1
