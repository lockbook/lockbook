//
//  NewLockbookView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 2/9/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct CreateAccountView: View {
    var lockbookApi: LockbookApi
    @State private var username: String = ""
    @State private var showingAlert = false
    @EnvironmentObject var screenCoordinator: ScreenCoordinator

    var body: some View {
        VStack {
            TextField("username", text: $username)
                .autocapitalization(.none)
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .multilineTextAlignment(.center)
                .padding(50)
                
            MonokaiButton(text: "Create Account")
                .onTapGesture {
                    if (self.lockbookApi.createAccount(username: self.username)) {
                        self.screenCoordinator.currentView = .listView
                    } else {
                        self.showingAlert = true
                    }
                }
        }
        .navigationBarTitle("New Lockbook")
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to create account!"))
        }
    }
}

struct CreateAccountView_Previews: PreviewProvider {
    static var previews: some View {
        CreateAccountView(lockbookApi: FakeApi()).environmentObject(ScreenCoordinator(files: []))
    }
}
