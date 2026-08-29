@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.annotation.SuppressLint
import android.content.pm.ActivityInfo
import android.content.res.Configuration
import android.os.Bundle
import android.view.View
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityUpgradeAccountBinding
import app.lockbook.model.AlertModel
import kotlinx.coroutines.*
import net.lockbook.Lb
import net.lockbook.LbError
import timber.log.Timber
import java.lang.ref.WeakReference

class UpgradeAccountActivity : AppCompatActivity() {
    private lateinit var binding: ActivityUpgradeAccountBinding

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

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

        binding.exitBilling.setOnClickListener {
            finish()
        }

        (application as App).billingClientLifecycle.apply {
            billingEvent.observe(this@UpgradeAccountActivity) { billingEvent ->
                handleBillingEvent(billingEvent)
            }
            premiumPrice.observe(this@UpgradeAccountActivity) { price ->
                binding.premiumPrice.text = getString(R.string.per_month_price, price)
            }
        }

        binding.subscribeToPlan.isEnabled = true
        binding.subscribeToPlan.setOnClickListener {
            (application as App).billingClientLifecycle.launchBillingFlow(this)
        }
    }

    private fun handleBillingEvent(billingEvent: BillingEvent) {
        when (billingEvent) {
            BillingEvent.NotifyUnrecoverableError -> {
                alertModel.notify(resources.getString(R.string.unrecoverable_billing_error)) {
                    finish()
                }
            }

            BillingEvent.SuccessfulPurchase -> {
                alertModel.notifySuccessfulPurchaseConfirm {
                    setResult(SUCCESSFUL_SUBSCRIPTION_PURCHASE)
                    this@UpgradeAccountActivity.finish()
                }
            }

            is BillingEvent.GooglePlayPurchase -> {
                lifecycleScope.launch {
                    binding.progressOverlay.visibility = View.VISIBLE
                    binding.subscribeToPlan.isEnabled = false

                    try {
                        withContext(Dispatchers.IO) {
                            Lb.upgradeAccountGooglePlay(billingEvent.purchaseToken, billingEvent.accountId)
                        }

                        alertModel.notifySuccessfulPurchaseConfirm {
                            setResult(SUCCESSFUL_SUBSCRIPTION_PURCHASE)
                            this@UpgradeAccountActivity.finish()
                        }
                    } catch (err: LbError) {
                        alertModel.notifyError(err)
                    } catch (err: CancellationException) {
                        throw err
                    } catch (err: Throwable) {
                        Timber.e(err, "Unexpected Google Play purchase confirmation error")
                        alertModel.notifyBasicError()
                    } finally {
                        binding.progressOverlay.visibility = View.GONE
                        binding.subscribeToPlan.isEnabled = true
                    }
                }
            }

            is BillingEvent.NotifyError -> {
                alertModel.notifyError(billingEvent.error)
            }

            is BillingEvent.NotifyErrorMsg -> {
                alertModel.notifyWithToast(billingEvent.error)
            }
        }
    }
}

const val SUCCESSFUL_SUBSCRIPTION_PURCHASE = 1
