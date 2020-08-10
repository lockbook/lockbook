//
//  LoginManager.swift
//  ios
//
//  Created by Raayan Pillai on 7/6/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import Foundation

class LoginManager: ObservableObject {
    var lockbookApi: LockbookApi
    @Published var account: Account?
    
    init() {
        self.lockbookApi = FakeApi()
    }
    init(lockbookApi: LockbookApi) {
        self.lockbookApi = lockbookApi
        let result = lockbookApi.getAccount()
        switch result {
        case .success(let account):
            self.account = account
        case .failure(let error):
            print("No account! \(error)")
        }
    }

    func createAccount(username: String) -> Account? {
        switch self.lockbookApi.createAccount(username: username) {
        case .success(let account):
            self.account = account
            return account
        case .failure(let err):
            print("Account create failed with error: \(err)")
            return nil
        }
    }

    func importAccount(accountString: String) -> Account? {
        switch self.lockbookApi.importAccount(accountString: accountString) {
        case .success(let account):
            self.account = account
            return account
        case .failure(let err):
            print("Account import failed with error: \(err)")
            return nil
        }
    }
}
