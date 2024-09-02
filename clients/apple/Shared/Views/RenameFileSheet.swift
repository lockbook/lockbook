import Foundation
import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace

struct RenameFileSheet: View {
        
    let renamingFileInfo: RenamingFileInfo
    
    @State var newName: String = DI.sheets.renamingFileInfo?.name ?? ""
    @State var maybeError: String? = nil
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
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
                }
                .buttonStyle(.plain)
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
                .modifier(DisableAutoCapitalization())
            
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
            
            #if os(iOS)
            Spacer()
            #endif
                        
        }.renameFolderSheetFrame()
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

struct RenameFileSheet_Previews: PreviewProvider {
    static var previews: some View {
        Rectangle()
            .foregroundStyle(.white)
            .sheet(isPresented: Binding.constant(true), content: {
                RenameFileSheet(info: RenamingFileInfo(name: "Apple", maybeParent: nil))
                    .presentationDetents([.height(150)])
                    .presentationDragIndicator(.visible)
            })
    }
}

extension View {
    @ViewBuilder
    public func renameFolderSheetFrame() -> some View {
        #if os(macOS)
        self.padding(20).frame(width: 320)
        #else
        self.padding(20)
        #endif
    }
}
