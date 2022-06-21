package app.lockbook.screen

import android.animation.ObjectAnimator
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.widget.LinearLayout
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityUpgradeAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.coroutines.*
import java.lang.ref.WeakReference

class UpgradeAccountActivity : AppCompatActivity() {

    private var _binding: ActivityUpgradeAccountBinding? = null
    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    enum class AccountTier {
        Free,
        PremiumMonthly,
    }

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!
    var selectedTier = AccountTier.Free

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityUpgradeAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

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

        animateTierSelectionToggle(selectedTierCardView, true)
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
                        val confirmResult =
                            CoreModel.upgradeAccountGooglePlay(billingEvent.purchaseToken, billingEvent.accountId)
                        withContext(Dispatchers.Main) {

                            when (confirmResult) {
                                is Ok -> {
                                    binding.progressOverlay.visibility = View.GONE
                                    binding.subscribeToPlan.isEnabled = true

                                    alertModel.notifySuccessfulPurchaseConfirm {
                                        setResult(SUCCESSFUL_SUBSCRIPTION_PURCHASE)
                                        this@UpgradeAccountActivity.finish()
                                    }
                                }
                                is Err -> alertModel.notifyError(
                                    confirmResult.error.toLbError(
                                        applicationContext.resources
                                    )
                                )
                            }
                        }
                    }
                }
            }
            is BillingEvent.NotifyError -> alertModel.notifyError(billingEvent.error)
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
        animateTierSelectionToggle(oldTierCardView, false)
        animateTierSelectionToggle(tierCardView as LinearLayout, true)
    }

    private fun launchPurchaseFlow(selectedTier: AccountTier) {
        if (selectedTier == AccountTier.PremiumMonthly) {
            (application as App).billingClientLifecycle.launchBillingFlow(this, selectedTier)
        }
    }

    private fun animateTierSelectionToggle(linearLayout: LinearLayout, selected: Boolean) {
        val color = if (selected) ResourcesCompat.getColor(resources, R.color.lightBlue, null) else Color.TRANSPARENT

        ObjectAnimator.ofArgb(linearLayout, "backgroundColor", color).apply {
            duration = 100
            start()
        }
    }

    companion object {
        const val SELECTED_TIER_KEY = "selected_tier_key"
    }
}

const val SUCCESSFUL_SUBSCRIPTION_PURCHASE = 1
