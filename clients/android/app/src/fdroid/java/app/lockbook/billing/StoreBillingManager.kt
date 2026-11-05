package app.lockbook.billing

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.lifecycleScope
import app.lockbook.R
import app.lockbook.util.SingleMutableLiveData
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.LbError
import timber.log.Timber

/** Builds without Google Play pay on Stripe's checkout page in the browser. */
class StoreBillingManager(
    applicationContext: Context,
) : BillingManager {
    private val _billingEvent = SingleMutableLiveData<BillingEvent>()
    private val _premiumPrice = MutableLiveData(applicationContext.getString(R.string.premium_price))

    override val billingEvent: LiveData<BillingEvent>
        get() = _billingEvent

    override val premiumPrice: LiveData<String>
        get() = _premiumPrice

    override fun launchBillingFlow(activity: Activity) {
        val appCompatActivity = activity as? AppCompatActivity
        if (appCompatActivity == null) {
            _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            return
        }

        appCompatActivity.lifecycleScope.launch {
            try {
                val url = withContext(Dispatchers.IO) { Lb.stripeCheckoutUrl(false) }
                activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
            } catch (error: CancellationException) {
                throw error
            } catch (error: LbError) {
                _billingEvent.value = BillingEvent.NotifyError(error)
            } catch (error: Throwable) {
                Timber.e(error, "Could not open Stripe checkout")
                _billingEvent.value = BillingEvent.NotifyUnrecoverableError
            }
        }
    }
}
