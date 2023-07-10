import Foundation
import SwiftUI
import SwiftLockbookCore

struct NewFolderSheet: View {
        
    @State var folderName: String = ""
    @State var maybeError: String? = nil
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        if let creatingFolderInfo = DI.sheets.creatingFolderInfo {
            VStack (alignment: .leading, spacing: 15){
                HStack (alignment: .center) {
                    Text("New folder")
                        .bold()
                        .font(.title)
                    Spacer()
                    Button(action: { presentationMode.wrappedValue.dismiss() }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.gray)
                            .imageScale(.large)
                            .frame(width: 50, height: 50, alignment: .center)
                    }
                }
                HStack {
                    Text("Inside:")
                    Text(creatingFolderInfo.parentPath)
                        .font(.system(.body, design: .monospaced))
                }
                
                TextField("Choose a filename", text: $folderName, onCommit: {
                    onCommit(creatingFolderInfo: creatingFolderInfo)
                })
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    .autocapitalization(.none)
                
                if let error = maybeError {
                    Text(error)
                        .foregroundColor(.red)
                        .bold()
                }
                
                Spacer()
            }.newFolderSheetFrameForMacOS()
        }
    }
    
    func onCommit(creatingFolderInfo: CreatingFolderInfo) {
        if let error = DI.files.createFolderSync(name: folderName, maybeParent: creatingFolderInfo.maybeParent) {
            maybeError = error
        } else {
            maybeError = nil
            presentationMode.wrappedValue.dismiss()
        }

        
    }
}

extension View {
    @ViewBuilder
    public func newFolderSheetFrameForMacOS() -> some View {
        #if os(macOS)
        self.padding(20).frame(width: 320, height: 150)
        #else
        self.padding(20)
        #endif
    }
}


