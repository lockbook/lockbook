//
//  EditorView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct EditorView: View {
    var lockbookApi: LockbookApi
    let metadata: FileMetadata
    
    var body: some View {
        VStack(alignment: .leading) {
            Text(metadata.name)
                .bold()
                .underline()
                .padding(.bottom, 10)
            Text("id: \(metadata.id)")
            Text("path: \(metadata.path)")
            Text("updatedAt: \(metadata.updatedAt)")
            Text("status: \(metadata.status.rawValue)")
        }
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        EditorView(lockbookApi: FakeApi(), metadata: FakeApi().fakeMetadatas.first!)
    }
}
