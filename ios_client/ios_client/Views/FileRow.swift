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
    var color: Color
    
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
                ZStack {
                    Rectangle()
                        .fill(self.color)
                        .frame(width: 100, height: 50)
                    Text(self.metadata.status.rawValue)
                        .foregroundColor(.white)
                        .bold()
                }
            }
        }
    }
    
    init(lockbookApi: LockbookApi, metadata: FileMetadata) {
        self.lockbookApi = lockbookApi
        self.metadata = metadata
        switch metadata.status {
            case .New: self.color = Color.purple
            case .Local: self.color = Color.blue
            case .Remote: self.color = Color.red
            case .Synced: self.color = Color.green
        }
    }
    
}

struct FileRow_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            ForEach(FakeApi().updateMetadata(sync: true)) { meta in
                FileRow(lockbookApi: FakeApi(), metadata: meta)
            }
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
