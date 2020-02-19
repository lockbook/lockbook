//
//  NewLockbookView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 2/9/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct NewLockbookView: View {
    
    @State private var username: String = ""
    
    var body: some View {
        VStack {
            HStack {
                Text("Username:")
                    .font(.callout)
                    .bold()
                TextField("", text: $username)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
            }.padding(.bottom, 50)
            
            MonokaiButton(text: "Create Account")
                .onTapGesture {
                    print(create_account(self.username))
            }
        }
        .navigationBarTitle("New Lockbook")
    }
    
    // Push view when that button is clicked and succeeds:
    // https://stackoverflow.com/questions/57315409/push-view-programmatically-in-callback-swiftui
    
    // make it so the new view that is pushed is the root:
    // https://stackoverflow.com/questions/58562063/create-a-navigationlink-without-back-button-swiftui
}

struct NewLockbookView_Previews: PreviewProvider {
    static var previews: some View {
        NewLockbookView()
    }
}
