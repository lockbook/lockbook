@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.billing

import android.app.Activity
import android.content.Context
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.R
import app.lockbook.util.SingleMutableLiveData
import app.lockbook.util.getString
import com.android.billingclient.api.*
import com.android.billingclient.api.BillingClient.BillingResponseCode
import timber.log.Timber
import java.util.*

class StoreBillingManager(
    private val applicationContext: Context,
) : BillingManager,
    PurchasesUpdatedListener,
    BillingClientStateListener,
    ProductDetailsResponseListener,
    PurchasesResponseListener {
    private val billingClient: BillingClient by lazy {
        val pendingPurchasesParams =
            PendingPurchasesParams
                .newBuilder()
                .enableOneTimeProducts()
                .build()

        BillingClient
            .newBuilder(applicationContext)
            .setListener(this)
            .enablePendingPurchases(pendingPurchasesParams)
            .enableAutoServiceReconnection()
            .build()
    }
    private var productDetails: ProductDetails? = null
    private val _billingEvent = SingleMutableLiveData<BillingEvent>()
    private val _premiumPrice = MutableLiveData<String>()

    override val billingEvent: LiveData<BillingEvent>
        get() = _billingEvent

    override val premiumPrice: LiveData<String>
        get() = _premiumPrice

    override fun onCreate(owner: LifecycleOwner) {
        if (!billingClient.isReady) {
            billingClient.startConnection(this)
        }
    }

    override fun showInAppMessaging(activity: Activity) {
        val inAppMessageParams =
            InAppMessageParams
                .newBuilder()
                .addInAppMessageCategoryToShow(InAppMessageParams.InAppMessageCategoryId.TRANSACTIONAL)
                .addAllInAppMessageCategoriesToShow()
                .build()

        billingClient.showInAppMessages(activity, inAppMessageParams) {}
    }

    override fun onDestroy(owner: LifecycleOwner) {
        if (billingClient.isReady) {
            billingClient.endConnection()
        }
    }

    override fun onBillingServiceDisconnected() {
        billingClient.startConnection(this)
    }

    override fun onBillingSetupFinished(billingResult: BillingResult) {
        Timber.i(billingResult.debugMessage)

        if (billingResult.responseCode == BillingResponseCode.OK) {
            val queryProductParams =
                QueryProductDetailsParams
                    .newBuilder()
                    .setProductList(
                        listOfProducts.map { productId ->
                            QueryProductDetailsParams.Product
                                .newBuilder()
                                .setProductId(productId)
                                .setProductType(BillingClient.ProductType.SUBS)
                                .build()
                        },
                    ).build()

            val queryPurchasesParams =
                QueryPurchasesParams
                    .newBuilder()
                    .setProductType(BillingClient.ProductType.SUBS)
                    .build()

            billingClient.queryProductDetailsAsync(queryProductParams, this)
            billingClient.queryPurchasesAsync(queryPurchasesParams, this)
        }
    }

    override fun onProductDetailsResponse(
        billingResult: BillingResult,
        queryProductDetailsResult: QueryProductDetailsResult,
    ) {
        val response = BillingResponse(billingResult.responseCode)
        val productDetailsList = queryProductDetailsResult.productDetailsList
        Timber.i(billingResult.debugMessage)

        when {
            response.isOk -> {
                if (productDetailsList.size == listOfProducts.size) {
                    productDetails = productDetailsList[0]
                    monthlyOffer(productDetailsList[0])
                        ?.pricingPhases
                        ?.pricingPhaseList
                        ?.lastOrNull()
                        ?.formattedPrice
                        ?.let(_premiumPrice::postValue)
                }
            }

            response.isUnrecoverableError -> {
                _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            }
        }
    }

    override fun launchBillingFlow(activity: Activity) {
        val billingFlowParams =
            billingFlowParamsBuilder()
                ?: return _billingEvent.postValue(BillingEvent.NotifyErrorMsg(getString(activity.resources, R.string.billing_not_ready)))

        val response = BillingResponse(billingClient.launchBillingFlow(activity, billingFlowParams).responseCode)

        when {
            response.isOk -> {}

            response.isRecoverableError -> {
                _billingEvent.postValue(BillingEvent.NotifyErrorMsg(getString(applicationContext.resources, R.string.billing_not_ready)))
            }

            else -> {
                _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            }
        }
    }

    private fun billingFlowParamsBuilder(): BillingFlowParams? {
        val offerToken = monthlyOffer(productDetails ?: return null)?.offerToken ?: return null

        return BillingFlowParams
            .newBuilder()
            .setProductDetailsParamsList(
                listOf(
                    BillingFlowParams.ProductDetailsParams
                        .newBuilder()
                        .setProductDetails(productDetails ?: return null)
                        .setOfferToken(offerToken)
                        .build(),
                ),
            ).setObfuscatedAccountId(UUID.randomUUID().toString())
            .build()
    }

    private fun monthlyOffer(details: ProductDetails): ProductDetails.SubscriptionOfferDetails? =
        details.subscriptionOfferDetails?.firstOrNull { PREMIUM_MONTHLY_OFFER_ID in it.offerTags }

    override fun onPurchasesUpdated(
        billingResult: BillingResult,
        purchases: MutableList<Purchase>?,
    ) {
        consumePurchase(billingResult, purchases)
    }

    override fun onQueryPurchasesResponse(
        billingResult: BillingResult,
        purchases: MutableList<Purchase>,
    ) {
        consumePurchase(billingResult, purchases)
    }

    private fun consumePurchase(
        billingResult: BillingResult,
        purchases: MutableList<Purchase>?,
    ) {
        val billingResponse = BillingResponse(billingResult.responseCode)
        Timber.i(billingResult.debugMessage)

        when {
            billingResponse.isOk &&
                purchases?.size == 1 &&
                purchases[0].purchaseState == Purchase.PurchaseState.PURCHASED &&
                purchases[0].accountIdentifiers?.obfuscatedAccountId?.isEmpty() == false -> {
                if (!purchases[0].isAcknowledged) {
                    _billingEvent.postValue(
                        BillingEvent.GooglePlayPurchase(
                            purchases[0].purchaseToken,
                            purchases[0].accountIdentifiers?.obfuscatedAccountId
                                ?: return _billingEvent.postValue(
                                    BillingEvent.NotifyErrorMsg(getString(applicationContext.resources, R.string.basic_error)),
                                ),
                        ),
                    )
                }
            }

            billingResponse.isUnrecoverableError -> {
                _billingEvent.postValue(BillingEvent.NotifyUnrecoverableError)
            }

            billingResponse.isCancelable || billingResponse.isOk -> {}

            else -> {
                _billingEvent.postValue(BillingEvent.NotifyErrorMsg(getString(applicationContext.resources, R.string.basic_error)))
            }
        }
    }

    companion object {
        private const val PREMIUM_PRODUCT_ID = "app.lockbook.premium_subscription"
        private const val PREMIUM_MONTHLY_OFFER_ID = "monthly"

        private val listOfProducts =
            listOf(
                PREMIUM_PRODUCT_ID,
            )
    }
}

@JvmInline
private value class BillingResponse(
    val code: Int,
) {
    val isOk: Boolean
        get() = code == BillingResponseCode.OK

    val isCancelable: Boolean
        get() =
            code in
                setOf(
                    BillingResponseCode.USER_CANCELED,
                )

    val isRecoverableError: Boolean
        get() =
            code in
                setOf(
                    BillingResponseCode.SERVICE_DISCONNECTED,
                    BillingResponseCode.ERROR,
                )

    val isUnrecoverableError: Boolean
        get() =
            code in
                setOf(
                    BillingResponseCode.SERVICE_UNAVAILABLE,
                    BillingResponseCode.BILLING_UNAVAILABLE,
                    BillingResponseCode.DEVELOPER_ERROR,
                    BillingResponseCode.ITEM_UNAVAILABLE,
                    BillingResponseCode.FEATURE_NOT_SUPPORTED,
                )
}
