import SwiftLockbookCore
import SwiftUI
import StoreKit

public enum StoreError: Error {
    case failedVerification
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

class BillingService: ObservableObject {
    let core: LockbookApi
    
    let products: [String: String]
    var monthlySubscription: Product? = nil
    var purchaseResult: PurchaseResult? = nil
    @Published var cancelSubscriptionResult: CancelSubscriptionResult? = nil
    
    init(_ core: LockbookApi) {
        self.core = core
        
        if let path = Bundle.main.path(forResource: "Products", ofType: "plist"),
        let plist = FileManager.default.contents(atPath: path) {
            products = (try? PropertyListSerialization.propertyList(from: plist, format: nil) as? [String: String]) ?? [:]
        } else {
            products = [:]
        }
        
        Task {
            await requestProducts()
        }
    }
    
    func listenForTransactions() -> Task<Void, Error> {
        return Task.detached {
            for await verification in Transaction.updates {
                do {
                    if let receipt = self.getReceipt() {
                        let transaction = try self.checkVerified(verification)
                            
                        let result = self.core.upgradeAccountAppStore(originalTransactionId: String(transaction.id), appAccountToken: transaction.appAccountToken!.uuidString, encodedReceipt: receipt)
                        
                        switch result {
                        case .success(_):
                            await transaction.finish()
                            self.purchaseResult = .success
                        case .failure(let error):
                            DI.errors.handleError(error)
                        }
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
            let result = try await monthlySubscription!.purchase(options: purchaseOpt)
            switch result {
            case .success(let verification):
                if let receipt = getReceipt() {
                    let transaction = try checkVerified(verification)
                        
//                    print("ITEMS: \(String(transaction.id)) \(transaction.appAccountToken) \(receipt)")
                    let result = self.core.upgradeAccountAppStore(originalTransactionId: String(transaction.id), appAccountToken: accountToken.uuidString, encodedReceipt: receipt)
                    
                    switch result {
                    case .success(_):
                        await transaction.finish()
                        purchaseResult = .success
                        return .success
                    case .failure(let error):
                        DI.errors.handleError(error)
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
        }
        catch {
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
            let storeProducts = try await Product.products(for: products.keys)

            if storeProducts.count == 1 {
                monthlySubscription = storeProducts[0]
            } else {
                print("No products!")
            }
            
            SKPaymentQueue.default().restoreCompletedTransactions()
        } catch {
            print("Failed product request from the App Store server: \(error)")
        }
    }
    
    func cancelSubscription() {
        DispatchQueue.global(qos: .userInteractive).async {
            let result = self.core.cancelSubscription()
            
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
