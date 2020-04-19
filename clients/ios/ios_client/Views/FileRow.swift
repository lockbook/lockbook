//
//  FileRow.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/12/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct FileRow: View {
    var metadata: FileMetadata
    var color: Color
    var image: Image
    @EnvironmentObject var screenCoordinator: Coordinator

    var body: some View {
        NavigationLink(destination: EditorView(screenCoordinator: self.screenCoordinator, metadata: metadata)) {
            HStack {
                VStack {
                    HStack {
                        Text(metadata.name)
                            .font(.headline)
                        Spacer()
                    }
                    HStack {
                        Text("Last synced \(intEpochToString(micros: metadata.version))")
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
    
    init(metadata: FileMetadata) {
        self.metadata = metadata
        switch metadata.status {
            case .New:
                self.color = Color.purple
                self.image = Image(systemName: "plus")
            case .Local:
                self.color = Color.blue
                self.image = Image(systemName: "tray.and.arrow.up")
            case .Remote:
                self.color = Color.red
                self.image = Image(systemName: "tray.and.arrow.down")
            case .Synced:
                self.color = Color.green
                self.image = Image(systemName: "arrow.2.circlepath")
        }
    }
    
}

struct FileRow_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            ForEach(FakeApi().updateMetadata()) { meta in
                FileRow(metadata: meta)
            }
        }
        .previewLayout(.fixed(width: 300, height: 50))
    }
}
