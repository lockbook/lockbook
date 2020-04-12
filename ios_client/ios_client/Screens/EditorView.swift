//
//  EditorView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct EditorView: View {
    let metadata: FileMetadata
    
    var body: some View {
        VStack {
            Text(metadata.name)
                .bold()
                .underline()
            HStack {
                Text("id: \(metadata.id)")
                Text("path: \(metadata.path)")
            }
            Text("updatedAt: \(metadata.updatedAt)")
            Text("status: \(metadata.status)")
        }
    }
}

struct EditorView_Previews: PreviewProvider {
    static var previews: some View {
        EditorView(metadata: FileMetadata(id: "abcdef", name: "testfile.md", path: "/some/place", updatedAt: 10000, status: "Remote"))
    }
}
