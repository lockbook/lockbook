import StoreKit
import SwiftUI
import SwiftWorkspace

class BillingState: ObservableObject {
    static let MONTHLY_SUBSCRIPTION_PRODUCT_ID = "basic.premium"
    static let PREMIUM_DATA_CAP: Double = 30_000_000_000

    var processPending: Task<Void, Error>?
    var subProduct: Product?

    var purchaseState: PurchaseState = .uninitiated

    init() {
        processPendingTransactions()
    }

    func processPendingTransactions() {
        processPending = Task.detached { [self] in
            await listenToTransactions()
        }
    }

    func requestProducts() async {
        guard
            let storeProducts = try? await Product.products(for: [
                BillingState.MONTHLY_SUBSCRIPTION_PRODUCT_ID,
            ])
        else {
            return
        }

        if storeProducts.count == 1 {
            subProduct = storeProducts[0]
        }
    }

    func listenToTransactions() async {
        if subProduct == nil {
            await requestProducts()
        }

        if subProduct == nil {
            return
        }

        for await verificationRes in Transaction.updates {
            await processTransactionUpdate(
                verificationRes: verificationRes
            )
        }
    }

    func processTransactionUpdate(
        verificationRes: VerificationResult<StoreKit.Transaction>
    ) async {
        guard case let .verified(transaction) = verificationRes else {
            return
        }

        if case let .success(.some(info)) = AppState.lb.getSubscriptionInfo(),
           info.isPremium() == true
        {
            return
        }

        let originalID = String(transaction.originalID)
        guard let appAccountToken = transaction.appAccountToken else {
            return
        }

        let res = AppState.lb.upgradeAccountAppStore(
            originalTransactionId: originalID,
            appAccountToken: appAccountToken.uuidString.lowercased()
        )

        switch res {
        case .success():
            await transaction.finish()
        case let .failure(err):
            if err.code == .appStoreAccountAlreadyLinked {
                await transaction.finish()
            }
        }
    }

    func launchPurchasePremium() {
        Task {
            await purchasePremium()
        }
    }

    func purchasePremium() async {
        await MainActor.run { purchaseState = .processing }

        if subProduct == nil {
            await requestProducts()
        }

        if subProduct == nil {
            await MainActor.run { purchaseState = .failure }
            return
        }

        guard
            let res = try? await subProduct!.purchase(options: [
                Product.PurchaseOption.appAccountToken(UUID()),
            ])
        else {
            await MainActor.run { purchaseState = .failure }

            return
        }

        switch res {
        case let .success(verificationRes):
            await MainActor.run { purchaseState = .success }
            await processTransactionUpdate(
                verificationRes: verificationRes
            )
        case .pending:
            await MainActor.run { purchaseState = .pending }
        case .userCancelled:
            await MainActor.run { purchaseState = .uninitiated }
        default:
            await MainActor.run { purchaseState = .failure }
        }
    }
}

enum PurchaseState {
    case uninitiated
    case processing
    case pending
    case success
    case failure
}

extension BillingState {
    static var preview: BillingState {
        BillingState()
    }
}
