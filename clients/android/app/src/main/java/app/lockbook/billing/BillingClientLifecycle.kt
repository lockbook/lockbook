package app.lockbook.billing

import android.app.Activity
import android.content.Context
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LiveData
import app.lockbook.screen.UpgradeAccountActivity
import app.lockbook.util.LbError
import app.lockbook.util.SingleMutableLiveData
import com.android.billingclient.api.*
import com.android.billingclient.api.BillingClient.BillingResponseCode

class BillingClientLifecycle private constructor(
    private val applicationContext: Context
) : DefaultLifecycleObserver,
    PurchasesUpdatedListener,
    BillingClientStateListener,
    ProductDetailsResponseListener {

    private lateinit var billingClient: BillingClient
    private var productDetails: ProductDetails? = null

    private val _billingEvent = SingleMutableLiveData<BillingEvent>()

    val billingEvent: LiveData<BillingEvent>
        get() = _billingEvent

    override fun onCreate(owner: LifecycleOwner) {
        billingClient = BillingClient.newBuilder(applicationContext)
            .setListener(this)
            .enablePendingPurchases()
            .build()

        if (!billingClient.isReady) {
            billingClient.startConnection(this)
        }
    }

    override fun onDestroy(owner: LifecycleOwner) {
        if (billingClient.isReady) {
            billingClient.endConnection()
        }
    }

    override fun onPurchasesUpdated(
        billingResult: BillingResult,
        purchases: MutableList<Purchase>?
    ) {
        when (billingResult.responseCode) {
            BillingResponseCode.OK -> {
                when {
                    purchases?.size == 1 && purchases[0].purchaseToken == PREMIUM_PRODUCT_ID -> {
                        _billingEvent.postValue(BillingEvent.SuccessfulPurchase(purchases[0].purchaseToken))
                    }
                    else -> {
                        _billingEvent.postValue(BillingEvent.NotifyError(LbError.basicError(applicationContext.resources)))
                    }
                }
            }
            BillingResponseCode.USER_CANCELED -> _billingEvent.postValue(BillingEvent.Canceled)
            else -> _billingEvent.postValue(BillingEvent.NotifyError(LbError.basicError(applicationContext.resources)))
        }
    }

    override fun onBillingServiceDisconnected() {
        billingClient.startConnection(this)
    }

    override fun onBillingSetupFinished(billingResult: BillingResult) {
        if (billingResult.responseCode == BillingResponseCode.OK) {
            val queryProductParams = QueryProductDetailsParams.newBuilder().setProductList(
                LIST_OF_PRODUCTS.map { productId ->
                    QueryProductDetailsParams.Product.newBuilder()
                        .setProductId(productId)
                        .setProductType(BillingClient.ProductType.SUBS)
                        .build()
                }
            ).build()

            billingClient.queryProductDetailsAsync(queryProductParams, this)
        }
    }

    override fun onProductDetailsResponse(billingResult: BillingResult, productDetailsList: MutableList<ProductDetails>) {
        val response = BillingResponse(billingResult.responseCode)

        when {
            response.isOk -> {
                if (productDetailsList.size == LIST_OF_PRODUCTS.size) {
                    productDetails = productDetailsList[0]
                }
            }
            response.isUnRecoverableError -> _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            else -> {}
        }
    }

    fun launchBillingFlow(activity: Activity, newTier: UpgradeAccountActivity.AccountTier) {
        val billingFlowParams = billingFlowParamsBuilder(newTier)
            ?: return _billingEvent.postValue(BillingEvent.NotifyError(LbError.basicError(applicationContext.resources)))

        val response = BillingResponse(billingClient.launchBillingFlow(activity, billingFlowParams).responseCode)

        when {
            response.isOk -> {}
            response.isRecoverableError -> _billingEvent.postValue(BillingEvent.NotifyError(LbError.basicError(applicationContext.resources)))
            else -> _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
        }
    }

    private fun getSubscriptionOffers(): List<ProductDetails.SubscriptionOfferDetails>? = productDetails?.subscriptionOfferDetails

    private fun billingFlowParamsBuilder(newTier: UpgradeAccountActivity.AccountTier): BillingFlowParams? {
        val offerTag = when (newTier) {
            UpgradeAccountActivity.AccountTier.Free -> return null
            UpgradeAccountActivity.AccountTier.PremiumMonthly -> PREMIUM_MONTHLY_OFFER_ID
        }

        val offerToken = getSubscriptionOffers()?.filter { it.offerTags[0] == offerTag }?.map { it.offerToken }?.get(0) ?: return null

        return BillingFlowParams.newBuilder()
            .setProductDetailsParamsList(
                listOf(
                    BillingFlowParams.ProductDetailsParams.newBuilder()
                        .setProductDetails(productDetails ?: return null)
                        .setOfferToken(offerToken)
                        .build()
                )
            )
            .build()
    }

    companion object {
        const val PREMIUM_PRODUCT_ID = "app.lockbook.premium_subscription"

        const val PREMIUM_MONTHLY_OFFER_ID = "monthly"

        private val LIST_OF_PRODUCTS = listOf(
            PREMIUM_PRODUCT_ID
        )

        @Volatile
        private var INSTANCE: BillingClientLifecycle? = null

        fun getInstance(applicationContext: Context): BillingClientLifecycle =
            INSTANCE ?: synchronized(this) {
                INSTANCE ?: BillingClientLifecycle(applicationContext).also { INSTANCE = it }
            }
    }
}

sealed class BillingEvent {
    data class SuccessfulPurchase(val purchaseToken: String) : BillingEvent()
    object Canceled : BillingEvent()
    object NotifyUnrecoverableError : BillingEvent()
    data class NotifyError(val error: LbError) : BillingEvent()
}

@JvmInline
private value class BillingResponse(val code: Int) {
    val isOk: Boolean
        get() = code == BillingResponseCode.OK

    val isRecoverableError: Boolean
        get() = code in setOf(
            BillingResponseCode.ERROR,
            BillingResponseCode.SERVICE_DISCONNECTED,
        )

    val isUnRecoverableError: Boolean
        get() = code in setOf(
            BillingResponseCode.SERVICE_UNAVAILABLE,
            BillingResponseCode.BILLING_UNAVAILABLE,
            BillingResponseCode.DEVELOPER_ERROR,
            BillingResponseCode.ITEM_UNAVAILABLE,
            BillingResponseCode.FEATURE_NOT_SUPPORTED,
        )
}
