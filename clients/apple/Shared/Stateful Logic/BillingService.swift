import SwiftLockbookCore
import SwiftUI
import StoreKit

public enum StoreError: Error {
    case failedVerification
}

class BillingService: ObservableObject {

    let core: LockbookApi
    let products: [String: String]
    
    var monthlySubscription: Product? = nil
    
    var isPremium: Bool? = nil
    
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
    
    func purchasePremium() async throws -> StoreKit.Transaction? {
        let accountToken = UUID()
        let purchaseOpt: Set<Product.PurchaseOption> = [Product.PurchaseOption.appAccountToken(accountToken)]
        
        if let appStoreReceiptURL = Bundle.main.appStoreReceiptURL,
            FileManager.default.fileExists(atPath: appStoreReceiptURL.path) {

            do {
                let receiptData = try Data(contentsOf: appStoreReceiptURL, options: .alwaysMapped)
                
                
            }
            catch { print("Couldn't read receipt data with error: " + error.localizedDescription) }
        }
        
        let result = try await monthlySubscription!.purchase(options: purchaseOpt)

        switch result {
        case .success(let verification):
            let transaction = try checkVerified(verification)
            await transaction.finish()
            
            return transaction
        case .userCancelled, .pending:
            return nil
        default:
            return nil
        }
    }
    
    func checkVerified<T>(_ result: VerificationResult<T>) throws -> T {
        switch result {
        case .unverified:
            throw StoreError.failedVerification
        case .verified(let safe):
            return safe
        }
    }
    
    func requestProducts() async {
        do {
            let storeProducts = try await Product.products(for: products.keys)

            monthlySubscription = storeProducts[0]
        } catch {
            print("Failed product request from the App Store server: \(error)")
        }
    }
}
