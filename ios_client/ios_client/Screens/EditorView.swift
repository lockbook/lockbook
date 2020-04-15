//
//  EditorView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct EditorView: View {
    let lockbookApi: LockbookApi
    let metadata: FileMetadata
    @State var content: String
    @State private var showingAlert = false

    var body: some View {
        VStack {
            MultilineTextView(text: self.$content)
            Button(action: {
                if (self.lockbookApi.updateFile(id: self.metadata.id, content: self.content)) {
                    print("Updated \(self.metadata)")
                    
                } else {
                    self.showingAlert = true
                }
            }) {
                HStack {
                    Image(systemName: "bolt")
                    Text("Reload")
                    Image(systemName: "bolt")
                }
            }
            VStack(alignment: .leading) {
                Text("id: \(metadata.id)")
                Text("path: \(metadata.path)")
                Text("updatedAt: \(metadata.updatedAt)")
                Text("status: \(metadata.status.rawValue)")
            }
        }
        .navigationBarTitle(metadata.name)
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to create file!"))
        }
    }
    
    init(lockbookApi: LockbookApi, metadata: FileMetadata) {
        self.lockbookApi = lockbookApi
        self.metadata = metadata
        if let file = lockbookApi.getFile(id: metadata.id) {
            self._content = State.init(initialValue: file.content)
        } else {
            self._content = State.init(initialValue: "")
        }
    }
}

struct MultilineTextView: UIViewRepresentable {
    @Binding var text: String

    func makeUIView(context: Context) -> UITextView {
        let view = UITextView()
        view.isScrollEnabled = true
        view.isEditable = true
        view.isUserInteractionEnabled = true
        return view
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        uiView.text = text
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            EditorView(lockbookApi: FakeApi(), metadata: FakeApi().fakeMetadatas.first!)
        }
    }
}
