//
//  FileRow.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/12/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI
import SwiftLockbookCore

struct DocumentRow: View {
    @ObservedObject var coordinator: Coordinator
    var metadata: FileMetadata
    var image: Image

    var body: some View {
        NavigationLink(destination: FileView(coordinator: self.coordinator, metadata: metadata)) {
            HStack {
                self.image
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
    
    init(coordinator: Coordinator, metadata: FileMetadata) {
        self.coordinator = coordinator
        self.metadata = metadata
        switch (false, false, false, metadata.deleted) {
            case (true, _, _, _):
                self.image = Image(systemName: "plus")
            case (_, true, _, _):
                self.image = Image(systemName: "tray.and.arrow.down")
            case (_, _, true, _):
                self.image = Image(systemName: "tray.and.arrow.up")
            case (_, _, _, true):
                self.image = Image(systemName: "trash")
            case (_, _, _, _):
                self.image = Image(systemName: "doc")
        }
    }
    
}

struct FileRow_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            ForEach(FakeApi().fileMetas) { meta in
                DocumentRow(coordinator: Coordinator(), metadata: meta)
            }
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
