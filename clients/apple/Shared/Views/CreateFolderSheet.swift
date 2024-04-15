import Foundation
import SwiftUI
import SwiftLockbookCore

struct CreateFolderSheet: View {
    
    var creatingFolderInfo: CreatingFolderInfo
        
    @State var folderName: String = ""
    @State var maybeError: String? = nil
    
    @Environment(\.presentationMode) var presentationMode
        
    var body: some View {
        VStack (alignment: .leading, spacing: 15){
            
            HStack (alignment: .center) {
                Text("Create folder")
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
                
                Text(creatingFolderInfo.parentPath)
                    .font(.system(.body, design: .monospaced))
            }
            
            TextField("Choose a filename", text: $folderName, onCommit: {
                onCommit()
            })
                .textFieldStyle(RoundedBorderTextFieldStyle())
                .autocapitalization(.none)
            
            if let error = maybeError {
                Text(error)
                    .foregroundColor(.red)
                    .bold()
            }
            
            Button(action: {
                onCommit()
            }, label: {
                Text("Create")
            })
            .buttonStyle(.borderedProminent)
            
            #if os(iOS)
            Spacer()
            #endif

        }.createFolderSheetFrame()
    }
    
    func onCommit() {
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
    public func createFolderSheetFrame() -> some View {
        #if os(macOS)
        self.padding(20).frame(width: 320)
        #else
        self.padding(20)
        #endif
    }
}


