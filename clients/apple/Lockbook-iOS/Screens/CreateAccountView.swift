//
//  NewLockbookView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 2/9/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct CreateAccountView: View {
    @State private var username: String = ""
    @State private var showingAlert = false
    @ObservedObject var loginManager: LoginManager

    var body: some View {
        VStack {
            TextField("username", text: $username)
                .autocapitalization(.none)
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .multilineTextAlignment(.center)
                .padding(50)
                
            MonokaiButton(text: "Create Account")
                .onTapGesture {
                    if let account = self.loginManager.createAccount(username: self.username) {
                        print("Created account \(account)")
                    }
                }
        }
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to create account!"))
        }
    }
}

struct CreateAccountView_Previews: PreviewProvider {
    static var previews: some View {
        CreateAccountView(loginManager: LoginManager())
    }
}
