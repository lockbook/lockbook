package app.lockbook.screen

import android.animation.ObjectAnimator
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.widget.LinearLayout
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.res.ResourcesCompat
import app.lockbook.R
import app.lockbook.databinding.ActivityUpgradeAccountBinding


class UpgradeAccountActivity: AppCompatActivity() {

    private var _binding: ActivityUpgradeAccountBinding? = null

    enum class AccountTier {
        Free,
        PremiumMonthly,
        PremiumYearly,
    }

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!
    var selectedTier = AccountTier.Free

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityUpgradeAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        if(savedInstanceState != null) {
            selectedTier = AccountTier.valueOf(savedInstanceState.getString(SELECTED_TIER_KEY, AccountTier.Free.name))
        }

        binding.switchAccountTierFree.setOnClickListener(clickListener)
        binding.switchAccountTierPremiumMonthly.setOnClickListener(clickListener)
        binding.switchAccountTierPremiumYearly.setOnClickListener(clickListener)

        binding.exitBilling.setOnClickListener {
            finish()
//            overridePendingTransition(R.anim.slide_in, R.anim.slide_out)
        }

        val selectedTierCardView = when(selectedTier) {
            AccountTier.Free -> binding.switchAccountTierFree
            AccountTier.PremiumMonthly -> binding.switchAccountTierPremiumMonthly
            AccountTier.PremiumYearly -> binding.switchAccountTierPremiumYearly
        }

        animateTierSelectionToggle(selectedTierCardView, true)
        binding.subscribeToPlan.isEnabled = selectedTier != AccountTier.Free
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putString(SELECTED_TIER_KEY, selectedTier.name)

        super.onSaveInstanceState(outState)
    }

    fun toggleSubscribeButton(oldSelectedTier: AccountTier, newSelectedTier: AccountTier) {
//        if(oldSelectedTier == AccountTier.Free && newSelectedTier != AccountTier.Free) {
//            binding.subscribeToPlan.visibility = View.VISIBLE
//        } else if(oldSelectedTier != AccountTier.Free && newSelectedTier == AccountTier.Free) {
//            binding.subscribeToPlan.visibility = View.GONE
//        }
        binding.subscribeToPlan.isEnabled = selectedTier != AccountTier.Free
    }

    private val clickListener = View.OnClickListener { tierCardView ->
        val oldSelectedTier = selectedTier

        selectedTier = when(tierCardView) {
            binding.switchAccountTierFree -> AccountTier.Free
            binding.switchAccountTierPremiumMonthly -> AccountTier.PremiumMonthly
            binding.switchAccountTierPremiumYearly -> AccountTier.PremiumYearly
            else -> AccountTier.Free
        }

        val oldTierCardView = when(oldSelectedTier) {
            AccountTier.Free -> binding.switchAccountTierFree
            AccountTier.PremiumMonthly -> binding.switchAccountTierPremiumMonthly
            AccountTier.PremiumYearly -> binding.switchAccountTierPremiumYearly
        }

        toggleSubscribeButton(oldSelectedTier, selectedTier)

        animateTierSelectionToggle(oldTierCardView, false)
        animateTierSelectionToggle(tierCardView as LinearLayout, true)
    }

    private fun animateTierSelectionToggle(linearLayout: LinearLayout, selected: Boolean) {
        val color = if(selected) ResourcesCompat.getColor(resources, R.color.lightBlue, null) else Color.TRANSPARENT

        ObjectAnimator.ofArgb(linearLayout, "backgroundColor", color).apply {
            duration = 100
            start()
        }
    }

    companion object {
        const val SELECTED_TIER_KEY = "selected_tier_key"
    }
}
