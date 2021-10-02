///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import SwiftLockbookCore
import UniformTypeIdentifiers

struct OutlineBranch: View {
    @EnvironmentObject var files: FileService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var errors: UnexpectedErrorService
    
    @ObservedObject var outlineState: OutlineState
    @StateObject var state: BranchState = BranchState()
    
    let file: ClientFileMetadata
    var level: CGFloat

    var children: [ClientFileMetadata] {
        files.files.filter {
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
                    if let isRenaming = outlineState.renaming, isRenaming == file {
                        SyntheticOutlineRow(
                            fileType: file.fileType,
                            level: level,
                            onCommit: { s in
                                outlineState.renaming = nil
                                files.renameFile(id: isRenaming.id, name: s)
                            },
                            onCancel: {
                                withAnimation {
                                    outlineState.renaming = nil
                                }
                            },
                            pendingImage: Image(systemName: "pencil"),
                            nameField: file.name
                        ).onDisappear {
                            withAnimation {
                                self.files.refresh()
                            }
                        }
                    } else {
                        if file == outlineState.selectedItem {
                            OutlineRow(file: file, level: level, open: $state.open, dragging: $outlineState.dragging)
                                .background(Color.accentColor)
                                .foregroundColor(Color.white)
                                .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                        } else {
                            OutlineRow(file: file, level: level, open: $state.open, dragging: $outlineState.dragging)
                                .onTapGesture {
                                    if file.fileType == .Folder {
                                        withAnimation {
                                            state.open.toggle()
                                        }
                                    } else {
                                        // Animating this causes editor to load weirdly
                                        outlineState.selectedItem = self.file
                                        print("tap")
                                    }
                                }
                        }
                    }
                }
                if isLeaf == false && (state.open == true || level == -1) {
                    ForEach(children) { child in
                        OutlineBranch(outlineState: outlineState, file: child, level: self.level + 1)
                    }
                }
                state.creating.map { c in
                    SyntheticOutlineRow(
                        fileType: c,
                        level: self.level + 1,
                        onCommit: handleCreate(meta: file, type: c),
                        onCancel: {
                            withAnimation {
                                state.creating = nil
                            }
                        },
                        pendingImage: Image(systemName: "plus")
                    )
                    .id(1)
                    .onAppear {
                        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100)) {
                            withAnimation {
                                scrollView.scrollTo(1, anchor: .center)
                            }
                        }
                    }
                }
            }
            .contextMenu(menuItems: {
                OutlineContextMenu(outlineState: outlineState, branchState: state, meta: file)
            })
            .onDrop(of: [UTType.text], delegate: DragDropper(file: file, current: $outlineState.dragging, open: $state.open, moveFile: { drag in
                files.moveFile(id: drag.id, newParent: self.file.id)
            }))
        }
    }
    
    func handleDelete(meta: ClientFileMetadata) -> () -> Void {
        return {
            files.deleteFile(id: meta.id)
        }
    }
    
    func handleCreate(meta: ClientFileMetadata, type: FileType) -> (String) -> Void {
        return { creatingName in
            switch DI.core.createFile(name: creatingName, dirId: meta.id, isFolder: type == .Folder) {
            case .success(let newMeta):
                doneCreating()
                files.refresh()
                status.checkForLocalWork()
                if (newMeta.fileType == .Document) {
                    outlineState.selectedItem = newMeta
                }
            case .failure(let err):
                errors.handleError(err)
            }
        }
    }
    
    func doneCreating() {
        withAnimation {
            state.creating = .none
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
