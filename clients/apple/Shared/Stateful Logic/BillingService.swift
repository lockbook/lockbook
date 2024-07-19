import SwiftLockbookCore
import SwiftUI
import StoreKit

public enum StoreError: Error {
    case failedVerification
    case noProduct
}

public enum PurchaseResult {
    case success
    case pending
    case failure
    case inFlow
}

public enum CancelSubscriptionResult {
    case success
    case appstoreActionRequired
}


let MONTHLY_SUBSCRIPTION_PRODUCT_ID = "basic.premium"

class BillingService: ObservableObject {
    let core: LockbookApi
    
    var pendingTransactionsListener: Task<Void, Error>? = nil

    var maybeMonthlySubscription: Product? = nil
    @Published var purchaseResult: PurchaseResult? = nil
    @Published var cancelSubscriptionResult: CancelSubscriptionResult? = nil
    
    var showPurchaseToast: Bool = false
    
    @Published var showManageSubscriptionView: Bool = false
    
    init(_ core: LockbookApi) {
        self.core = core
    }
    
    deinit {
        pendingTransactionsListener?.cancel()
    }
    
    func launchBillingBackgroundTasks() {
        Task {
            await requestProducts()
            pendingTransactionsListener = listenForTransactions()
        }
    }
    
    func listenForTransactions() -> Task<Void, Error> {
        return Task.detached {
            for await verification in Transaction.updates {
                
                guard let transaction = self.checkVerified(verification) else {
                    return
                }
                
                switch self.core.getUsage() {
                case .success(let usages):
                    if usages.dataCap.exact == FREE_TIER_USAGE_CAP {
                        guard let appAccountToken = transaction.appAccountToken else {
                            DI.errors.errorWithTitle("Billing Error", "An unexpected error has occurred.")
                            return
                        }
                            
                        let result = self.core.newAppleSub(originalTransactionId: String(transaction.originalID), appAccountToken: appAccountToken.uuidString.lowercased())
                                
                        switch result {
                        case .success(_):
                            await transaction.finish()
                        case .failure(let error):
                            if error.kind == .UiError(.AppStoreAccountAlreadyLinked) {
                                await transaction.finish()
                                self.purchaseResult = .success
                            }
                        }
                    } else {
                        await transaction.finish()
                    }
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }

    func purchasePremium() {
        purchaseResult = .inFlow
        
        Task {
            let purchaseOpt: Set<Product.PurchaseOption> = [Product.PurchaseOption.appAccountToken(UUID())]
            
            if case .none = maybeMonthlySubscription {
                launchBillingBackgroundTasks()
            }
            
            guard let monthlySubscription = maybeMonthlySubscription else {
                await updatePurchaseResult(.failure)
                return
            }
            
            guard let result = try? await monthlySubscription.purchase(options: purchaseOpt) else {
                await updatePurchaseResult(.failure)
                return
            }
            
            switch result {
            case .success(let verification):
                guard let transaction = checkVerified(verification) else {
                    await updatePurchaseResult(.failure)
                    return
                }
                    
                guard let accountToken = transaction.appAccountToken?.uuidString else {
                    await updatePurchaseResult(.failure)
                    return
                }
                    
                let result = self.core.newAppleSub(originalTransactionId: String(transaction.originalID), appAccountToken: accountToken.lowercased())
                    
                switch result {
                case .success(_):
                    showPurchaseToast = true
                    await transaction.finish()
                    await updatePurchaseResult(.success)
                case .failure(let error):
                    await updatePurchaseResult(.failure)
                    DI.errors.handleError(error)
                }
            case .pending:
                showPurchaseToast = true
                await updatePurchaseResult(.pending)
            case .userCancelled:
                await updatePurchaseResult(nil)
            default:
                await updatePurchaseResult(.failure)
            }
        }
    }
    
    func updatePurchaseResult(_ newValue: PurchaseResult?) async {
        await MainActor.run {
            purchaseResult = newValue
        }
    }
    
    func checkVerified<T>(_ result: VerificationResult<T>) -> T? {
        switch result {
        case .unverified:
            return nil
        case .verified(let safe):
            return safe
        }
    }
    
    func requestProducts() async {
        do {
            let storeProducts = try await Product.products(for: [MONTHLY_SUBSCRIPTION_PRODUCT_ID])

            if storeProducts.count == 1 {
                maybeMonthlySubscription = storeProducts[0]
            } else {
                DI.errors.errorWithTitle("App Store Error", "Cannot retrieve data from the app store (err_id: 2).")
            }
        } catch {
            DI.errors.errorWithTitle("App Store Error", "Cannot retrieve data from the app store (err_id: 1).")
        }
    }
    
    func cancelSubscription() {
        DispatchQueue.global(qos: .userInteractive).async {
            let result = self.core.cancelSub()
            
            DispatchQueue.main.async {
                switch result {
                case .success(_):
                    self.cancelSubscriptionResult = .success
                case .failure(let err):
                    switch err.kind {
                    case .UiError(let errorVariant):
                        if errorVariant == .CannotCancelForAppStore {
                            self.cancelSubscriptionResult = .appstoreActionRequired
                        } else {
                            DI.errors.handleError(err)
                        }
                    case .Unexpected:
                        DI.errors.handleError(err)
                    }
                }
            }
        }
    }
}
