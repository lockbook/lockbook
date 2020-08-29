//
//  NewLockbookView.swift
//  ios_client
//
//  Created by Parth Mehrotra on 2/9/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct CreateFileView: View {
    @State private var fileName = ""
    @State private var isFolder = false
    @State private var errorMessage: String?
    @State private var alerting = false
    @ObservedObject var coordinator: Coordinator
    @Environment(\.presentationMode) var presentationMode: Binding<PresentationMode>

    var body: some View {
        VStack {
            TextField("name", text: $fileName)
                .autocapitalization(.none)
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .multilineTextAlignment(.center)
                .padding(.horizontal, 50)
            
            Toggle("Folder?", isOn: $isFolder)
                .padding(.horizontal, 80)
            
            MonokaiButton(text: "Create \(isFolder ? "Folder":"Document")")
                .onTapGesture {
                    if !self.fileName.contains("/") {
                        if self.coordinator.createFile(name: self.fileName, isFolder: self.isFolder) {
                            self.presentationMode.wrappedValue.dismiss()
                            let _ = self.coordinator.navigateAndListFiles(dirId: self.coordinator.currentId)
                        } else {
                            self.errorMessage = "Could not create file!"
                            self.alerting = true
                        }
                    } else {
                        self.errorMessage = "File must not contain '/'"
                        self.alerting = true
                    }
                }
                .padding(.vertical, 40)
                
        }
        .alert(isPresented: $alerting) {
            Alert(title: Text(errorMessage ?? "No error message!"))
        }
    }
}

struct CreateFileView_Previews: PreviewProvider {
    static var previews: some View {
        CreateFileView(coordinator: Coordinator())
    }
}
