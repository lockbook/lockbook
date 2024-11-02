import SwiftUI
import SwiftWorkspace

struct MoveSheet: View {
    
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var sheets: SheetState

    @Environment(\.presentationMode) var presentationMode
    
    let meta: File
    
    var body: some View {
        let root = fileService.files.first(where: { $0.parent == $0.id })!
        let wc = WithChild(root, fileService.files, { $0.id == $1.parent && $0.id != $1.id && $1.type == .folder })
        
        ScrollView {
            VStack {
                Text("Moving \(meta.name)").font(.headline)
                
                NestedList(
                    node: wc,
                    row: { dest in
                        Button(action: {
                            fileService.moveFile(id: meta.id, newParent: dest.id)
                            presentationMode.wrappedValue.dismiss()
                        }, label: {
                            Label(dest.name, systemImage: "folder")
                        })
                    }
                )
                Spacer()
            }.padding()
        }
    }
}

