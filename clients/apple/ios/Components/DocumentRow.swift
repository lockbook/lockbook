//
//  FileRow.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/12/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct DocumentRow: View {
    @ObservedObject var coordinator: Coordinator
    var metadata: FileMetadata
    var color: Color
    var image: Image

    var body: some View {
        NavigationLink(destination: FileView(coordinator: self.coordinator, metadata: metadata)) {
            HStack {
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
                Spacer()
                ZStack {
                    self.image
                        .foregroundColor(self.color)
                        .frame(width: 50, height: 30)
                }
            }
        }
    }
    
    init(coordinator: Coordinator, metadata: FileMetadata) {
        self.coordinator = coordinator
        self.metadata = metadata
        switch (false, false, false, metadata.deleted) {
            case (true, _, _, _):
                self.color = Color.green
                self.image = Image(systemName: "plus")
            case (_, true, _, _):
                self.color = Color.purple
                self.image = Image(systemName: "tray.and.arrow.down")
            case (_, _, true, _):
                self.color = Color.blue
                self.image = Image(systemName: "tray.and.arrow.up")
            case (_, _, _, true):
                self.color = Color.red
                self.image = Image(systemName: "trash")
            case (_, _, _, _):
                self.color = Color.primary
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
