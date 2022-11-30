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
    
    var updateListenerTask: Task<Void, Error>? = nil

    let products: [String: String]
    var monthlySubscription: Product? = nil
    var purchaseResult: PurchaseResult? = nil
    @Published var cancelSubscriptionResult: CancelSubscriptionResult? = nil
    
    var makingPurchaseAttempt = false
    
    init(_ core: LockbookApi) {
        self.core = core
        
        if let path = Bundle.main.path(forResource: "Products", ofType: "plist"),
        let plist = FileManager.default.contents(atPath: path) {
            products = (try? PropertyListSerialization.propertyList(from: plist, format: nil) as? [String: String]) ?? [:]
        } else {
            products = [:]
        }
        
        updateListenerTask = listenForTransactions()

        Task {
            await requestProducts()
        }
    }
    
    deinit {
        updateListenerTask?.cancel()
    }
    
    func listenForTransactions() -> Task<Void, Error> {
        return Task.detached {
            for await verification in Transaction.updates {
                do {
                    print("Attempting...")
                    if let receipt = self.getReceipt(), !self.makingPurchaseAttempt {
                        print("DOING...")
                        let transaction = try self.checkVerified(verification)
                            
                        let result = self.core.upgradeAccountAppStore(originalTransactionId: String(transaction.originalID), appAccountToken: transaction.appAccountToken!.uuidString.lowercased(), encodedReceipt: receipt)
                        
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
            SKReceiptRefreshRequest().start()
            
            switch result {
            case .success(let verification):
                while true {
                    if let receipt = getReceipt() {
                        makingPurchaseAttempt = true
                        let transaction = try checkVerified(verification)
                        
                        //                    print("ITEMS: \(String(transaction.id)) \(transaction.appAccountToken) \(receipt)")
                        print("SENDING IT TO CORE YK YK")
                        let result = self.core.upgradeAccountAppStore(originalTransactionId: String(transaction.originalID), appAccountToken: accountToken.uuidString.lowercased(), encodedReceipt: receipt)
                        
                        switch result {
                        case .success(_):
                            await transaction.finish()
                            print("FINISHED 1")
                            makingPurchaseAttempt = false
                            purchaseResult = .success
                            print("FINISHED 2")
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
                print("GOT HERE 3")
                return nil
            default:
                print("GOT HERE 4")
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
