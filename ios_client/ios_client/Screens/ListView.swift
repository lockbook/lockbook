//
//  ListView.swift
//  ios_client
//
//  Created by Raayan Pillai on 4/11/20.
//  Copyright © 2020 Lockbook. All rights reserved.
//

import SwiftUI

struct ListView: View {
    var lockbookApi: CoreApi
    @State private var files: [FileMetadata]
    
    var body: some View {
        VStack {
            NavigationView {
                List {
                    ForEach(files) { file in
                        NavigationLink(destination: EditorView(metadata: file)) {
                            Text(file.name)
                        }
                    }
                }
                .navigationBarTitle("Files")
            }
            MonokaiButton(text: "Reload Files")
                .onTapGesture {
                    let files = self.lockbookApi.get_files()
                    print(files)
                    self.files = files
                }
        }
    }
    
    init(lockbookApi: CoreApi) {
        self.lockbookApi = lockbookApi
        self._files = State(initialValue: lockbookApi.get_files())
    }
}

struct ListView_Previews: PreviewProvider {
    static var previews: some View {
        ListView(lockbookApi: CoreApi())
    }
}
