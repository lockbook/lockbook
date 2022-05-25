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
import app.lockbook.billing.BillingClientLifecycle
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityUpgradeAccountBinding
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.util.Animate
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.android.material.bottomsheet.BottomSheetDialog
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
    private val originTier = AccountTier.Free
    var selectedTier = AccountTier.Free

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityUpgradeAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        if (savedInstanceState != null) {
            selectedTier = AccountTier.valueOf(savedInstanceState.getString(SELECTED_TIER_KEY, AccountTier.Free.name))
        }
        
        binding.switchAccountTierFree.setOnClickListener(clickListener)
        binding.switchAccountTierPremiumMonthly.setOnClickListener(clickListener)

        binding.exitBilling.setOnClickListener {
            finish()
        }

        (application as App).billingClientLifecycle.billingEvent.observe(this) { billingEvent ->
            handleBillingEvent(billingEvent)
        }

        val selectedTierCardView = when (selectedTier) {
            AccountTier.Free -> binding.switchAccountTierFree
            AccountTier.PremiumMonthly -> binding.switchAccountTierPremiumMonthly
        }

        animateTierSelectionToggle(selectedTierCardView, true)
        binding.subscribeToPlan.isEnabled = selectedTier != AccountTier.Free
        binding.subscribeToPlan.setOnClickListener {
            launchPurchaseFlow(selectedTier)
        }
    }

    private fun handleBillingEvent(billingEvent: BillingEvent) {
        when (billingEvent) {
            BillingEvent.Canceled -> {}
            BillingEvent.NotifyUnrecoverableError -> alertModel.notify(resources.getString(R.string.unrecoverable_billing_error))
            is BillingEvent.SuccessfulPurchase -> {
                uiScope.launch {
                    Animate.animateVisibility(binding.progressOverlay, View.VISIBLE, 102, 500)

                    withContext(Dispatchers.IO) {
                        val confirmResult =
                            CoreModel.confirmAndroidSubscription(billingEvent.purchaseToken)
                        withContext(Dispatchers.Main) {

                            when (confirmResult) {
                                is Ok -> {
                                    Animate.animateVisibility(
                                        binding.progressOverlay,
                                        View.GONE,
                                        0,
                                        500
                                    )

                                    val successfulPurchaseDialog =
                                        BottomSheetDialog(this@UpgradeAccountActivity)
                                    successfulPurchaseDialog.setContentView(R.layout.purchased_premium)
                                    successfulPurchaseDialog.show()
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
            binding.switchAccountTierFree -> AccountTier.Free
            binding.switchAccountTierPremiumMonthly -> AccountTier.PremiumMonthly
            else -> AccountTier.Free
        }

        val oldTierCardView = when (oldSelectedTier) {
            AccountTier.Free -> binding.switchAccountTierFree
            AccountTier.PremiumMonthly -> binding.switchAccountTierPremiumMonthly
        }

        toggleSubscribeButton()
        animateTierSelectionToggle(oldTierCardView, false)
        animateTierSelectionToggle(tierCardView as LinearLayout, true)
    }

    private fun launchPurchaseFlow(selectedTier: AccountTier) {
        if (originTier != selectedTier) {
            if (selectedTier == AccountTier.Free) {
                CoreModel.cancelSubscription()
            } else {
                BillingClientLifecycle.getInstance(this).launchBillingFlow(this, selectedTier)
            }
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
