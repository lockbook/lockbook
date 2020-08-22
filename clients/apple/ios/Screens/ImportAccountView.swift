//
//  ImportAccountView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ImportAccountView: View {
    @State private var accountString: String = ""
    @State private var showingAlert = false
    @ObservedObject var loginManager: LoginManager

    var body: some View {
        VStack {
            TextField("account string", text: $accountString)
               .autocapitalization(.none)
               .textFieldStyle(RoundedBorderTextFieldStyle())
               .multilineTextAlignment(.center)
                .padding(.horizontal, 50)
                .padding(.bottom, 25)
           
            MonokaiButton(text: "Load Account")
                .onTapGesture {
                    if let account = self.loginManager.importAccount(accountString: self.accountString) {
                        print("Imported account: \(account)")
                        switch self.loginManager.lockbookApi.synchronize() {
                        case .success(_):
                            self.showingAlert = false
                        case .failure(let error):
                            print("Import failed with error: \(error)")
                            self.showingAlert = true
                        }
                    }
                }
        }
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to import account!"))
        }
    }
}

struct ImportAccountView_Previews: PreviewProvider {
    static var previews: some View {
        ImportAccountView(loginManager: LoginManager()).environmentObject(Coordinator())
    }
}
