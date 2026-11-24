package app.lockbook.billing

import android.app.Activity
import android.content.Context
import android.content.Intent
import androidx.appcompat.app.AppCompatActivity
import androidx.core.net.toUri
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.ui.UsageBarPreference
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.LbError
import timber.log.Timber

/** Builds without Google Play pay on Stripe's hosted pages in the browser. */
class StoreBillingManager(
    applicationContext: Context,
) : BillingManager {
    private val _billingEvent = SingleMutableLiveData<BillingEvent>()
    private val _premiumPrice = MutableLiveData(applicationContext.getString(R.string.premium_price))
    private var awaitingCheckout = false
    private var requestInFlight = false
    private var checkoutReturnJob: Job? = null

    override val billingEvent: LiveData<BillingEvent>
        get() = _billingEvent

    override val premiumPrice: LiveData<String>
        get() = _premiumPrice

    override val supportsExternalBillingPortal: Boolean
        get() = true

    override fun launchBillingFlow(activity: Activity) {
        if (requestInFlight) return
        val appCompatActivity = activity as? AppCompatActivity
        if (appCompatActivity == null) {
            _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            return
        }

        requestInFlight = true
        appCompatActivity.lifecycleScope.launch {
            try {
                val url = withContext(Dispatchers.IO) { Lb.stripeCheckoutUrl() }
                activity.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
                awaitingCheckout = true
            } catch (error: CancellationException) {
                throw error
            } catch (error: LbError) {
                if (error.kind == LbError.LbEC.AlreadyPremium) {
                    awaitingCheckout = false
                    _billingEvent.value = BillingEvent.SuccessfulPurchase
                } else {
                    _billingEvent.value = BillingEvent.NotifyError(error)
                }
            } catch (error: Throwable) {
                Timber.e(error, "Could not open Stripe checkout")
                _billingEvent.value = BillingEvent.NotifyUnrecoverableError
            } finally {
                requestInFlight = false
            }
        }
    }

    override fun launchBillingPortal(activity: Activity) {
        if (requestInFlight) return
        val appCompatActivity = activity as? AppCompatActivity
        if (appCompatActivity == null) {
            _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            return
        }

        requestInFlight = true
        appCompatActivity.lifecycleScope.launch {
            try {
                val url = withContext(Dispatchers.IO) { Lb.stripeBillingPortalUrl() }
                activity.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
            } catch (error: CancellationException) {
                throw error
            } catch (error: LbError) {
                _billingEvent.value = BillingEvent.NotifyError(error)
            } catch (error: Throwable) {
                Timber.e(error, "Could not open Stripe billing portal")
                _billingEvent.value = BillingEvent.NotifyUnrecoverableError
            } finally {
                requestInFlight = false
            }
        }
    }

    override fun checkCheckoutReturn(activity: Activity) {
        if (!awaitingCheckout) return
        val appCompatActivity = activity as? AppCompatActivity ?: return
        if (checkoutReturnJob?.isActive == true) return

        checkoutReturnJob =
            appCompatActivity.lifecycleScope.launch {
                // Give Stripe's webhook a few seconds to land after returning from the browser.
                for (attempt in 0 until 5) {
                    try {
                        val usage = withContext(Dispatchers.IO) { Lb.refreshUsage() }
                        if (usage.dataCap?.exact == UsageBarPreference.PAID_TIER_USAGE_BYTES) {
                            awaitingCheckout = false
                            _billingEvent.value = BillingEvent.SuccessfulPurchase
                            return@launch
                        }
                    } catch (error: CancellationException) {
                        throw error
                    } catch (error: Throwable) {
                        Timber.w(error, "Could not refresh usage after returning from Stripe checkout")
                    }
                    if (attempt < 4) {
                        delay(1500L)
                    }
                }
            }
    }
}
