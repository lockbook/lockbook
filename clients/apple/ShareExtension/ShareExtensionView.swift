//
//  ShareExtensionView.swift
//  ShareExtension
//
//  Created by Smail Barkouch on 7/31/24.
//

import Foundation
import SwiftUI
import SwiftLockbookCore

struct ShareExtensionView: View {
    
    @ObservedObject var shareModel: ShareViewModel
    
    var body: some View {
        VStack {
            if shareModel.downloadUbig {
                Text("Downloading... pleaase wait.")
                    .padding(.bottom)
                
                ProgressView()
            } else if shareModel.failed {
                Text("Failed to import.")
            } else if shareModel.finished {
                Text("Finished importing.")
            } else {
                Text("Importing...")
                
                ProgressView()
            }
        }
    }
}



