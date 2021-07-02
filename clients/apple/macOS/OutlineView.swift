///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import SwiftLockbookCore
import UniformTypeIdentifiers

struct OutlineBranch: View {
    @ObservedObject var core: GlobalState
    
    var file: ClientFileMetadata
    @Binding var selectedItem: ClientFileMetadata?
    var level: CGFloat
    @State var open: Bool = false
    @State var creating: FileType?
    @Binding var dragging: ClientFileMetadata?
    @Binding var renaming: ClientFileMetadata?

    var children: [ClientFileMetadata] {
        core.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    @ViewBuilder
    var body: some View {
        ScrollViewReader { scrollView in
            VStack(alignment: .leading, spacing: 2) { // spacing: 2 is what List uses
                if level == -1 {
                    Text(file.name).opacity(0.4)
                } else {
                    if let isRenaming = renaming, isRenaming == file {
                        SyntheticOutlineRow(
                            fileType: file.fileType,
                            level: level,
                            onCommit: { s in
                                renaming = nil
                                let r = core.api.renameFile(id: isRenaming.id, name: s)
                                if case .failure(let err) = r {
                                    core.handleError(err)
                                } else {
                                    withAnimation {
                                        core.updateFiles()
                                        core.checkForLocalWork()
                                    }
                                }
                            },
                            onCancel: {
                                withAnimation {
                                    renaming = nil
                                }
                            },
                            pendingImage: Image(systemName: "pencil"),
                            nameField: file.name
                        ).onDisappear {
                            withAnimation {
                                self.core.updateFiles()
                            }
                        }
                    } else {
                        if file == selectedItem {
                            OutlineRow(core: core, file: file, level: level, open: $open, dragging: $dragging)
                                .background(Color.accentColor)
                                .foregroundColor(Color.white)
                                .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                        } else {
                            OutlineRow(core: core, file: file, level: level, open: $open, dragging: $dragging)
                                .onTapGesture {
                                    if file.fileType == .Folder {
                                        withAnimation {
                                            self.open.toggle()
                                        }
                                    } else {
                                        // Animating this causes editor to load weirdly
                                        self.selectedItem = self.file
                                    }
                                }
                        }
                    }
                }
                if isLeaf == false && (open == true || level == -1) {
                    ForEach(children) { child in
                        OutlineBranch(core: core, file: child, selectedItem: self.$selectedItem, level: self.level + 1, dragging: self.$dragging, renaming: self.$renaming)
                    }
                }
                creating.map { c in
                    SyntheticOutlineRow(
                        fileType: c,
                        level: self.level + 1,
                        onCommit: handleCreate(meta: file, type: c),
                        onCancel: {
                            withAnimation {
                                creating = nil
                            }
                        },
                        pendingImage: Image(systemName: "plus")
                    )
                    .id(1)
                }
            }
            .contextMenu(menuItems: {
                makeContextActions(
                    meta: file,
                    creating: {
                        creating = $0
                        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100)) {
                            withAnimation {
                                scrollView.scrollTo(1, anchor: .center)
                            }
                        }
                    }
                )
                if (!file.isRoot) {
                    makeNonRootActions(
                        meta: file,
                        renaming: { renaming = file },
                        delete: handleDelete(meta: file)
                    )
                }
            })
            .onDrop(of: [UTType.text], delegate: DragDropper(file: file, current: $dragging, open: $open, moveFile: { drag in
                if case .failure(let err) = core.api.moveFile(id: drag.id, newParent: self.file.id) {
                    core.handleError(err)
                } else {
                    withAnimation {
                        core.updateFiles()
                    }
                    core.checkForLocalWork()
                }
            }))
        }
    }
    
    func handleDelete(meta: ClientFileMetadata) -> () -> Void {
        return {
            switch core.api.deleteFile(id: meta.id) {
            case .success(_):
                core.deleteChannel.send(meta)
                core.updateFiles()
                core.checkForLocalWork()
            case .failure(let err):
                core.handleError(err)
            }
        }
    }
    
    func handleCreate(meta: ClientFileMetadata, type: FileType) -> (String) -> Void {
        return { creatingName in
            switch core.api.createFile(name: creatingName, dirId: meta.id, isFolder: type == .Folder) {
            case .success(let newMeta):
                doneCreating()
                core.updateFiles()
                core.checkForLocalWork()
                if (newMeta.fileType == .Document) {
                    selectedItem = newMeta
                }
            case .failure(let err):
                core.handleError(err)
            }
        }
    }
    
    func doneCreating() {
        withAnimation {
            creating = .none
        }
    }
}


struct DragDropper: DropDelegate {
    let file: ClientFileMetadata
    @Binding var current: ClientFileMetadata?
    @Binding var open: Bool
    let moveFile: (ClientFileMetadata) -> Void

    init(file: ClientFileMetadata, current: Binding<ClientFileMetadata?>, open: Binding<Bool>, moveFile: @escaping (ClientFileMetadata) -> Void) {
        self.file = file
        self._current = current
        self._open = open
        self.moveFile = moveFile
    }

    func validateDrop(info: DropInfo) -> Bool {
        file.fileType == .Folder && current?.parent != file.id && current?.id != file.id
    }

    func dropEntered(info: DropInfo) {
        withAnimation {
            open = true
        }
    }

    func performDrop(info: DropInfo) -> Bool {
        if let toMove = current {
            moveFile(toMove)
        }
        current = nil
        return true
    }
}

struct OutlineSection: View {
    
    @ObservedObject var core: GlobalState
    
    var root: ClientFileMetadata
    @Binding var selectedItem: ClientFileMetadata?
    @State var dragging: ClientFileMetadata?
    @State var renaming: ClientFileMetadata?

    var children: [ClientFileMetadata] {
        core.files.filter {
            $0.parent == root.id && $0.id != root.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                // The padding in the section header is there to adjust for the inset hack.
                OutlineBranch(core: core, file: root, selectedItem: self.$selectedItem, level: -1, dragging: self.$dragging, renaming: self.$renaming)
                Spacer()
            }
            .listStyle(SidebarListStyle())
            .frame(minWidth: 10, maxWidth: .infinity, maxHeight: .infinity)
            .padding(8)
            // A hack for list row insets not working. This hack also applies to the section header though.
        }
    }
}

func makeContextActions(meta: ClientFileMetadata, creating: @escaping (FileType) -> Void) -> TupleView<(Text, Button<Label<Text, Image>>, Button<Label<Text, Image>>)> {
    TupleView((
        Text(meta.name),
        Button(action: { creating(.Document) }) {
            Label("Create a document", systemImage: "doc")
        },
        Button(action: { creating(.Folder) }) {
            Label("Create a folder", systemImage: "folder")
        }
    ))
}

func makeNonRootActions(meta: ClientFileMetadata, renaming: @escaping () -> Void, delete: @escaping () -> Void) -> TupleView<(Button<Label<Text, Image>>, Button<Label<Text, Image>>)> {
    TupleView((
        Button(action: renaming) {
            Label("Rename", systemImage: "pencil")
        },
        Button(action: delete) {
            Label("Delete", systemImage: "trash.fill")
        }
    ))
}

