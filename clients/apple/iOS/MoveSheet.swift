import SwiftUI
import SwiftLockbookCore

struct MoveSheet: View {
    
    let meta: DecryptedFileMetadata?
    
    @EnvironmentObject var fileService: FileService
    
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        if let meta = meta {
            let root = fileService.files.first(where: { $0.parent == $0.id })!
            let wc = WithChild(root, fileService.files, { $0.id == $1.parent && $0.id != $1.id && $1.fileType == .Folder })
            
            ScrollView {
                VStack {
                    Text("Moving \(meta.decryptedName)").font(.headline)
                    NestedList(
                        node: wc,
                        row: { dest in
                            Button(action: {
                                fileService.moveFile(id: meta.id, newParent: dest.id)
                                presentationMode.wrappedValue.dismiss()
                            }, label: {
                                Label(dest.decryptedName, systemImage: "folder")
                            })
                        }
                    )
                    Spacer()
                }.padding()
            }
        }
    }
}

struct NestedList<T: Identifiable, V: View>: View {
    let node: WithChild<T>
    let row: (T) -> V
    @State var expanded: Bool
    
    init(node: WithChild<T>, row: @escaping (T) -> V) {
        self.node = node
        self.row = row
        // Start expanded up to 3 levels deep!
        self._expanded = .init(initialValue: node.level < 3)
    }
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                row(node.value)
                Spacer()
                if (!node.children.isEmpty) {
                    Image(systemName: "chevron.right")
                        .rotationEffect(expanded ? .degrees(90) : .zero)
                        .onTapGesture {
                            withAnimation {
                                expanded.toggle()
                            }
                        }
                }
            }
            if (expanded) {
                ForEach(node.children) { c in
                    NestedList(node: c, row: row).padding(.leading, 30)
                }
            }
        }
    }
}

struct WithChild<T>: Identifiable where T: Identifiable {
    var id: T.ID {
        value.id
    }
    
    let value: T
    let children: [WithChild<T>]
    let level: Int
    
    init(_ value: T, _ children: [WithChild<T>], level: Int = 0) {
        self.value = value
        self.children = children
        self.level = level
    }
    
    init(_ value: T, _ ts: [T], _ desc: (T, T) -> Bool, level: Int = 0) {
        self.value = value
        self.level = level
        self.children = ts.compactMap {
            if (desc(value, $0)) {
                return Self($0, ts, desc, level: level+1)
            } else {
                return nil
            }
        }
    }
}

struct NestedList_Previews: PreviewProvider {
    static var previews: some View {
        NestedList(
            node: .init(FakeApi.root, FakeApi.fileMetas, { $0.id == $1.parent && $0.id != $1.id && $1.fileType == .Folder }),
            row: {
                Label($0.decryptedName, systemImage: "folder")
            }
        )
    }
}
