//
//  EditorView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FileView: View {
    @ObservedObject var coordinator: Coordinator
    let metadata: FileMetadata
    @State var content: String
    @State private var showingAlert = false

    var body: some View {
        VStack {
            TextEditor(text: self.$content)
        }
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to get/update file!"))
        }
        .onAppear {
            if let file = self.coordinator.getFile(meta: self.metadata) {
                self.content = file.secret
            } else {
                print("Could not load \(self.metadata)")
            }
        }
        .onDisappear {
            if let file = self.coordinator.getFile(meta: self.metadata) {
                if file.secret != self.content {
                    if (self.coordinator.updateFile(id: self.metadata.id, content: self.content)) {
                        self.coordinator.sync()
                    } else {
                        self.showingAlert = true
                    }
                } else {
                    print("Files look the same, not updating")
                }
            }
        }
        .navigationBarTitle("\(metadata.name)")
    }
    
    init(coordinator: Coordinator, metadata: FileMetadata) {
        self.coordinator = coordinator
        self.metadata = metadata
        if let file = coordinator.getFile(meta: metadata) {
            self._content = State.init(initialValue: file.secret)
        } else {
            self._content = State.init(initialValue: "")
            showingAlert = true
        }
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileView(coordinator: Coordinator(), metadata: FakeApi().fileMetas.first!)
        }
        .environment(\.colorScheme, .dark)
    }
}
