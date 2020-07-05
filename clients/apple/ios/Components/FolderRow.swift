//
//  FolderRow.swift
//  ios
//
//  Created by Raayan Pillai on 7/5/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FolderRow: View {
    var metadata: FileMetadata
    @EnvironmentObject var coordinator: Coordinator

    var body: some View {
        NavigationLink(destination: FolderList(dirId: metadata.id, dirName: metadata.name)) {
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
            ForEach(FakeApi().sync()) { meta in
                FolderRow(metadata: meta)
            }
        }
        .previewLayout(.fixed(width: 300, height: 50))
        .environmentObject(Coordinator())
    }
}
