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
    @EnvironmentObject var screenCoordinator: ScreenCoordinator

    var body: some View {
        VStack {
            TextView(text: self.$content)
            VStack(alignment: .leading) {
                Text("id: \(metadata.id)")
                Text("path: \(metadata.path)")
                Text("updatedAt: \(intEpochToString(unixTime: metadata.updatedAt))")
                Text("version: \(intEpochToString(unixTime: metadata.version))")
                Text("status: \(metadata.status.rawValue)")
            }
        }
        .navigationBarTitle(metadata.name)
        .alert(isPresented: $showingAlert) {
            Alert(title: Text("Failed to get/update file!"))
        }
        .onAppear {
            print("Editor -- Appearing")
            if let file = self.lockbookApi.getFile(id: self.metadata.id) {
                self.content = file.content
            } else {
                print("Could not load \(self.metadata)")
            }
        }
        .onDisappear {
            print("Editor -- Disappearing")
            if let file = self.lockbookApi.getFile(id: self.metadata.id) {
                if file.content != self.content {
                    if (self.lockbookApi.updateFile(id: self.metadata.id, content: self.content)) {
                        print("Updated \(self.metadata)")
                        self.screenCoordinator.files = self.lockbookApi.updateMetadata()
                    } else {
                        self.showingAlert = true
                    }
                } else {
                    print("Files look the same, not updating")
                }
            }
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

func intEpochToString(unixTime: Int) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(unixTime))
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy/mm/dd hh:mm: a"
    return formatter.string(from: date)
    
}

struct TextView: UIViewRepresentable {
    @Binding var text: String

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeUIView(context: Context) -> UITextView {

        let myTextView = UITextView()
        myTextView.delegate = context.coordinator

        myTextView.isScrollEnabled = true
        myTextView.isEditable = true
        myTextView.isUserInteractionEnabled = true
        myTextView.backgroundColor = UIColor(white: 0.0, alpha: 0.05)

        return myTextView
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        uiView.text = text
    }

    class Coordinator : NSObject, UITextViewDelegate {

        var parent: TextView

        init(_ uiTextView: TextView) {
            self.parent = uiTextView
        }

        func textView(_ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String) -> Bool {
            return true
        }

        func textViewDidChange(_ textView: UITextView) {
            self.parent.text = textView.text
        }
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            EditorView(lockbookApi: FakeApi(), metadata: FakeApi().fakeMetadatas.first!).environmentObject(ScreenCoordinator(files: []))
        }
    }
}
