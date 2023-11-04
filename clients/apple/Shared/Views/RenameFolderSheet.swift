import Foundation
import SwiftUI
import SwiftLockbookCore

struct RenameFolderSheet: View {
        
    @State var newFolderName: String = DI.sheets.renamingFolderInfo?.name ?? ""
    @State var maybeError: String? = nil
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        if let renamingFolderInfo = DI.sheets.renamingFolderInfo {
            VStack (alignment: .leading, spacing: 15){
                HStack (alignment: .center) {
                    Text("Rename folder")
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
                    Text(renamingFolderInfo.parentPath)
                        .font(.system(.body, design: .monospaced))
                }
                
                TextField("Choose a filename", text: $newFolderName, onCommit: {
                    onCommit(renamingFolderInfo: renamingFolderInfo)
                })
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    .autocapitalization(.none)
                
                if let error = maybeError {
                    Text(error)
                        .foregroundColor(.red)
                        .bold()
                }
                
                Spacer()
            }.renameFolderSheetFrameForMacOS()
        }
    }
    
    func onCommit(renamingFolderInfo: RenamingFolderInfo) {
        if let error = DI.files.renameFileSync(id: renamingFolderInfo.id, name: newFolderName) {
            maybeError = error
        } else {
            maybeError = nil
            presentationMode.wrappedValue.dismiss()
        }
    }
}

extension View {
    @ViewBuilder
    public func renameFolderSheetFrameForMacOS() -> some View {
        #if os(macOS)
        self.padding(20).frame(width: 320, height: 150)
        #else
        self.padding(20)
        #endif
    }
}


