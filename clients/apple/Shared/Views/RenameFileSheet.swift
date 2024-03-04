import Foundation
import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace

struct RenameFileSheet: View {
        
    @State var newName: String = DI.sheets.renamingFileInfo?.name ?? ""
    @State var maybeError: String? = nil
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        if let renamingFileInfo = DI.sheets.renamingFileInfo {
            VStack (alignment: .leading, spacing: 15) {
                HStack (alignment: .center) {
                    Text("Rename file")
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
                    Text(renamingFileInfo.parentPath)
                        .font(.system(.body, design: .monospaced))
                }
                
                TextField("Choose a filename", text: $newName, onCommit: {
                    onCommit(renamingFileInfo: renamingFileInfo)
                })
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                    .autocapitalization(.none)
                
                if let error = maybeError {
                    Text(error)
                        .foregroundColor(.red)
                        .bold()
                }
                
                Button(action: {
                    onCommit(renamingFileInfo: renamingFileInfo)
                }, label: {
                    Text("Rename")
                })
                .buttonStyle(.borderedProminent)
                
                Spacer()
                
            }.renameFolderSheetFrameForMacOS()
        }
    }
    
    func onCommit(renamingFileInfo: RenamingFileInfo) {
        if let error = DI.files.renameFileSync(id: renamingFileInfo.id, name: newName) {
            maybeError = error
        } else {
            DI.workspace.fileOpCompleted = .Rename(id: renamingFileInfo.id, newName: newName)
            DI.files.refresh()
            
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


