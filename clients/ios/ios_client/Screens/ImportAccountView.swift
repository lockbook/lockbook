//
//  ImportAccountView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/19/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ImportAccountView: View {
    var lockbookApi: LockbookApi
    @State private var username: String = ""
    @State private var keyString: String = ""
    @State private var showingAlert = false
    @EnvironmentObject var screenCoordinator: ScreenCoordinator

    var body: some View {
        VStack {
            TextField("username", text: $username)
                .autocapitalization(.none)
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .multilineTextAlignment(.center)
                .padding(.horizontal, 50)
            TextField("key string", text: $keyString)
               .autocapitalization(.none)
               .textFieldStyle(RoundedBorderTextFieldStyle())
               .multilineTextAlignment(.center)
                .padding(.horizontal, 50)
                .padding(.bottom, 25)
           
            MonokaiButton(text: "Load Account")
                .onTapGesture {
                if (self.lockbookApi.importAccount(username: self.username, keyString: self.keyString)) {
                    self.screenCoordinator.files = self.lockbookApi.updateMetadata()
                   self.screenCoordinator.currentView = .listView
                } else {
                   self.showingAlert = true
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
        ImportAccountView(lockbookApi: FakeApi()).environmentObject(ScreenCoordinator(lockbookApi: FakeApi()))
    }
}
