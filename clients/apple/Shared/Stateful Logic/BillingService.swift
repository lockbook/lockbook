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
    var purchaseResult: PurchaseResult? = nil
    @Published var cancelSubscriptionResult: CancelSubscriptionResult? = nil
    
    var makingPurchaseAttempt = false
    
    
    init(_ core: LockbookApi) {
        self.core = core
        pendingTransactionsListener = listenForTransactions()

        Task {
            await requestProducts()
        }
    }
    
    deinit {
        pendingTransactionsListener?.cancel()
    }
    
    func listenForTransactions() -> Task<Void, Error> {
        return Task.detached {
            for await verification in Transaction.updates {
                do {
                    let transaction = try self.checkVerified(verification)
                    switch self.core.getUsage() {
                    case .success(let usages):
                        if usages.dataCap.exact == 1000000 {
                            if let receipt = self.getReceipt(), !self.makingPurchaseAttempt, transaction.id == transaction.originalID {
                                guard let appAccountToken = transaction.appAccountToken else {
                                    throw StoreError.failedVerification
                                }
                                
                                let result = self.core.newAppleSub(originalTransactionId: String(transaction.originalID), appAccountToken: appAccountToken.uuidString.lowercased(), encodedReceipt: receipt)
                                
                                switch result {
                                case .success(_):
                                    await transaction.finish()
                                    self.purchaseResult = .success
                                case .failure(let error):
                                    DI.errors.handleError(error)
                                }
                            }
                        } else {
                            await transaction.finish()
                        }
                    case .failure(let error):
                        DI.errors.handleError(error)
                    }
                } catch {
                    print("Transaction failed verification")
                }
            }
        }
    }

    
    func purchasePremium() async throws -> PurchaseResult? {
        let accountToken = UUID()
        let purchaseOpt: Set<Product.PurchaseOption> = [Product.PurchaseOption.appAccountToken(accountToken)]
        
        do {
            guard let monthlySubscription = maybeMonthlySubscription else {
                throw StoreError.noProduct
            }
            
            let result = try await monthlySubscription.purchase(options: purchaseOpt)
            SKReceiptRefreshRequest().start()
            
            switch result {
            case .success(let verification):
                
                while true {
                    if let receipt = getReceipt() {
                        makingPurchaseAttempt = true
                        let transaction = try checkVerified(verification)
                        
                        let result = self.core.newAppleSub(originalTransactionId: String(transaction.originalID), appAccountToken: accountToken.uuidString.lowercased(), encodedReceipt: receipt)
                        
                        switch result {
                        case .success(_):
                            await transaction.finish()
                            makingPurchaseAttempt = false
                            purchaseResult = .success
                            return .success
                        case .failure(let error):
                            makingPurchaseAttempt = false
                            DI.errors.handleError(error)
                            return .failure
                        }
                    }
                }
            case .pending:
                purchaseResult = .pending
                return .pending
            case .userCancelled:
                return nil
            default:
                return .failure
            }
        } catch {
            print("Couldn't read receipt data with error: " + error.localizedDescription)
        }

        return .failure
    }
    
    func checkVerified<T>(_ result: VerificationResult<T>) throws -> T {
        switch result {
        case .unverified:
            throw StoreError.failedVerification
        case .verified(let safe):
            return safe
        }
    }
    
    func getReceipt() -> String? {
        if let appStoreReceiptURL = Bundle.main.appStoreReceiptURL,
            FileManager.default.fileExists(atPath: appStoreReceiptURL.path) {

            do {
                return try Data(contentsOf: appStoreReceiptURL, options: .alwaysMapped).base64EncodedString()
            }
            catch {
                return nil
            }
        }
        
        return nil
    }
    
    func requestProducts() async {
        do {
            let storeProducts = try await Product.products(for: [MONTHLY_SUBSCRIPTION_PRODUCT_ID])

            if storeProducts.count == 1 {
                maybeMonthlySubscription = storeProducts[0]
            } else {
                print("No products!")
            }
            
        } catch {
            print("Failed product request from the App Store server: \(error)")
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
