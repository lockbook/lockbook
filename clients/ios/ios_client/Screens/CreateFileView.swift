//
//  NewLockbookView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 2/9/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct CreateFileView: View {
    var lockbookApi: LockbookApi
    @State private var fileName: String = ""
    @State private var showingAlert = false
    @EnvironmentObject var screenCoordinator: ScreenCoordinator
    @Environment(\.presentationMode) var presentationMode: Binding<PresentationMode>

    var body: some View {
        VStack {
            TextField("name", text: $fileName)
                .autocapitalization(.none)
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .multilineTextAlignment(.center)
                .padding(.horizontal, 50)
                
            MonokaiButton(text: "Create File")
                .onTapGesture {
                    if let file = self.lockbookApi.createFile(name: self.fileName) {
                        print("File created \(file)")
                        self.screenCoordinator.files = self.lockbookApi.updateMetadata()
                        self.presentationMode.wrappedValue.dismiss()
                    } else {
                        self.showingAlert = true
                    }
                }
        }
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to create file!"))
        }
    }
}

struct CreateFileView_Previews: PreviewProvider {
    static var previews: some View {
        CreateFileView(lockbookApi: FakeApi())
    }
}
