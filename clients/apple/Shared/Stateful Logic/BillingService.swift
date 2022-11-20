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

public enum SubscriptionStatus {
    case premiumStripe
    case premiumGooglePlay
    case premiumAppStore
    case free
}

class BillingService: ObservableObject {
    let core: LockbookApi
    
    @Published var subscriptionInfo: SubscriptionStatus?
    
    let products: [String: String]
    
    var monthlySubscription: Product? = nil
    
    var isPremium: Bool? = nil
    var purchaseResult: PurchaseResult? = nil
    
    init(_ core: LockbookApi) {
        self.core = core
        
        if let path = Bundle.main.path(forResource: "Products", ofType: "plist"),
        let plist = FileManager.default.contents(atPath: path) {
            products = (try? PropertyListSerialization.propertyList(from: plist, format: nil) as? [String: String]) ?? [:]
        } else {
            products = [:]
        }
        
        refreshSubscriptionStatus()
        
        Task {
            await requestProducts()
        }
    }
    
    
    func refreshSubscriptionStatus() {
        DispatchQueue.global(qos: .userInteractive).async {
            if DI.accounts.account == nil {
                print("No account yet, but tried to check subscription info, ignoring")
                return
            }
            
            let subscriptionInfo = self.core.getSubscriptionInfo()
            
            DispatchQueue.main.async {
                switch subscriptionInfo {
                case .success(let subscriptionInfo):
                    switch subscriptionInfo {
                    case .none:
                        self.subscriptionInfo = .free
                    case .some(let subscriptionInfo):
                        switch subscriptionInfo.paymentPlatform {
                        case .Stripe(cardLast4Digits: _):
                            self.subscriptionInfo = .premiumStripe
                        case .GooglePlay(accountState: _):
                            self.subscriptionInfo = .premiumGooglePlay
                        case .AppStore(accountState: _):
                            self.subscriptionInfo = .premiumAppStore
                        }
                    }
                case .failure(let error):
                    DI.errors.handleError(error)
                }
            }
        }
    }
    
    func listenForTransactions() -> Task<Void, Error> {
        return Task.detached {
            for await result in Transaction.updates {
                do {
                    let transaction = try self.checkVerified(result)

                    

                    await transaction.finish()
                } catch {
                    //StoreKit has a transaction that fails verification. Don't deliver content to the user.
                    print("Transaction failed verification")
                }
            }
        }
    }

    
    func purchasePremium() async throws -> PurchaseResult? {
        let accountToken = UUID()
        let purchaseOpt: Set<Product.PurchaseOption> = [Product.PurchaseOption.appAccountToken(accountToken)]
        
        
//        if let appStoreReceiptURL = Bundle.main.appStoreReceiptURL,
//            FileManager.default.fileExists(atPath: appStoreReceiptURL.path) {
//
//            do {
//                let receiptData = try Data(contentsOf: appStoreReceiptURL, options: .alwaysMapped)
//                print(receiptData)
//
//                let receiptString = receiptData.base64EncodedString(options: [])
//
//                print("The data \(receiptString)")
//            }
//            catch { print("Couldn't read receipt data with error: " + error.localizedDescription) }
//        } else {
//            print("No data :(")
//        }


            print("Got here 1")
            do {
                print("Got here 2")
                let result = try await monthlySubscription!.purchase(options: purchaseOpt)
                print("Got here 3")

                switch result {
                case .success(let verification):
                    
                    print("Got here 4")
                    
                    let receipt = getReceipt()
                    if receipt != nil {
//                        let transaction = try checkVerified(verification)
                        
                        print("ITEMS: \(receipt)")

                        // self.core.upgradeAccountAppStore(originalTransactionId: <#T##String#>, appAccountToken: <#T##String#>, encodedReceipt: <#T##String#>)
                        
//                        await transaction.finish()
                        purchaseResult = .success
                        return .success
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


        print("Failed here 3")

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
            
            SKPaymentQueue().restoreCompletedTransactions()
            print(getReceipt())
        } catch {
            print("Failed product request from the App Store server: \(error)")
        }
    }
}
