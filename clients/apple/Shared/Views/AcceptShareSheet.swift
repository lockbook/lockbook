import SwiftUI
import SwiftLockbookCore

struct AcceptShareSheet: View {
    
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var sheets: SheetState

    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        if let meta = sheets.acceptingShareInfo {
            let root = fileService.files.first(where: { $0.parent == $0.id })!
            let wc = WithChild(root, fileService.files, { $0.id == $1.parent && $0.id != $1.id && $1.fileType == .Folder })
            
            ScrollView {
                VStack {
                    Text("Accepting \(meta.name)").font(.headline)
                    NestedList(
                        node: wc,
                        row: { dest in
                            Button(action: {
                                settings.acceptShare(targetMeta: meta, parent: dest.id)
                                print("HERE 3")
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
}
