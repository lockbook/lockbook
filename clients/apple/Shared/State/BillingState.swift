import SwiftUI
import SwiftWorkspace
import StoreKit

class BillingState: ObservableObject {    
    static let MONTHLY_SUBSCRIPTION_PRODUCT_ID = "basic.premium"
    static let PREMIUM_DATA_CAP: Double = 30000000000
    
    var processPending: Task<Void, Error>? = nil
    var subProduct: Product? = nil
    
    var purchaseState: PurchaseState = .uninitiated
    
    func processPendingTransactions() {
        processPending = Task.detached { [self] in
            await self.listenToTransactions()
        }
    }
    
    func requestProducts() async {
        guard let storeProducts = try? await Product.products(for: [BillingState.MONTHLY_SUBSCRIPTION_PRODUCT_ID]) else {
            return
        }
        
        if storeProducts.count == 1 {
            subProduct = storeProducts[0]
        }
    }
    
    func listenToTransactions() async {
        if subProduct == nil {
            await self.requestProducts()
        }
        
        if subProduct == nil {
            return
        }
        
        for await verificationRes in Transaction.updates {
            await self.processTransactionUpdate(verificationRes: verificationRes)
        }
    }
    
    func processTransactionUpdate(verificationRes: VerificationResult<StoreKit.Transaction>) async {
        guard case let .verified(transaction) = verificationRes else {
            return
        }
        
        guard case .success(nil) = AppState.lb.getSubscriptionInfo() else {
            return
        }
        
        let originalID = String(transaction.originalID)
        guard let appAccountToken = transaction.appAccountToken else {
            return
        }

        let res = AppState.lb.upgradeAccountAppStore(originalTransactionId: originalID, appAccountToken: appAccountToken.uuidString.lowercased())
        
        switch res {
        case .success():
            await transaction.finish()
        case .failure(let err):
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
            await self.requestProducts()
        }
        
        if subProduct == nil {
            await MainActor.run { purchaseState = .failure }
            return
        }
        
        guard let res = try? await subProduct!.purchase(options: [Product.PurchaseOption.appAccountToken(UUID())]) else {
            await MainActor.run { purchaseState = .failure }
            
            return
        }
        
        switch res {
        case .success(let verificationRes):
            await MainActor.run { purchaseState = .success }
            await self.processTransactionUpdate(verificationRes: verificationRes)
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
