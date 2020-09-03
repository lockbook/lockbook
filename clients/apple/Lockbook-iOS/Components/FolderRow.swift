//
//  FolderRow.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI
import SwiftLockbookCore

struct FolderRow: View {
    @ObservedObject var coordinator: Coordinator
    var metadata: FileMetadata

    var body: some View {
        NavigationLink(destination: FolderList(coordinator: self.coordinator, dirId: metadata.id, dirName: metadata.name)) {
            HStack {
                Image(systemName: "folder")
                    .foregroundColor(.blue)
                    .frame(width: 30, height: 30)
                VStack {
                    HStack {
                        Text(metadata.name)
                            .font(.headline)
                        Spacer()
                    }
                    HStack {
                        Text("Last synced \(intEpochToString(epoch: metadata.metadataVersion))")
                            .font(.footnote)
                            .foregroundColor(.secondary)
                        Spacer()
                    }
                }
            }
        }
    }
}

struct FolderRow_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            ForEach(FakeApi().fileMetas) { meta in
                FolderRow(coordinator: Coordinator(), metadata: meta)
            }
        }
        .previewLayout(.fixed(width: 300, height: 50))
        .environmentObject(Coordinator())
    }
}
