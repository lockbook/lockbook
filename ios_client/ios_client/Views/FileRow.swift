//
//  FileRow.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/12/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FileRow: View {
    var lockbookApi: LockbookApi
    var metadata: FileMetadata
    
    var body: some View {
        NavigationLink(destination: EditorView(lockbookApi: lockbookApi, metadata: metadata)) {
            HStack {
                VStack {
                    HStack {
                        Text(metadata.name).bold()
                        Spacer()
                    }
                    HStack {
                        Text("location: \(metadata.path)")
                        Spacer()
                    }
                }
                Spacer()
            }
        }
    }
}

struct FileRow_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            ForEach(FakeApi().updateMetadata()) { meta in
                FileRow(lockbookApi: FakeApi(), metadata: meta)
            }
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
