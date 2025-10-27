import SwiftUI
import Combine
import SwiftWorkspace
import AppKit

struct FileTreeView: NSViewRepresentable {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    let treeView = FileTreeOutlineView()
        
    func makeCoordinator() -> Coordinator {
        Coordinator(treeView: treeView, homeState: homeState, filesModel: filesModel, workspaceInput: workspaceInput)
    }
    
    func makeNSView(context: Context) -> some NSView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.horizontalScrollElasticity = .none
        scrollView.hasHorizontalScroller = false
        scrollView.hasHorizontalRuler = false
        scrollView.drawsBackground = false

        treeView.dataSource = context.coordinator.dataSource
        treeView.delegate = context.coordinator.delegate
        treeView.stronglyReferencesItems = true
        treeView.autoresizesOutlineColumn = true
        treeView.headerView = nil
        treeView.usesAutomaticRowHeights = false
        treeView.allowsMultipleSelection = true
        treeView.allowsEmptySelection = true
        treeView.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        treeView.setDraggingSourceOperationMask(.copy, forLocal: false)
        treeView.setDraggingSourceOperationMask(.move, forLocal: true)
        treeView.registerForDraggedTypes([NSPasteboard.PasteboardType(FileTreeDataSource.REORDER_PASTEBOARD_TYPE), .fileURL])

        let column = NSTableColumn()
        column.resizingMask = .autoresizingMask
        column.minWidth = 100

        treeView.addTableColumn(column)
        treeView.outlineTableColumn = column
        
        scrollView.documentView = treeView
        
        return scrollView
    }
    
    func updateNSView(_ nsView: NSViewType, context: Context) {
        
    }
    
    class Coordinator {
        let dataSource: FileTreeDataSource
        let delegate: FileTreeDelegate
        
        let filesModel: FilesViewModel
        
        private var cancellables: Set<AnyCancellable> = []
        
        init(treeView: FileTreeOutlineView, homeState: HomeState, filesModel: FilesViewModel, workspaceInput: WorkspaceInputState) {
            self.filesModel = filesModel
            self.dataSource = FileTreeDataSource(homeState: homeState, filesModel: filesModel)
            self.delegate = FileTreeDelegate(homeState: homeState, filesModel: filesModel, workspaceInput: workspaceInput)
            
            homeState.workspaceOutput.$selectedFolder.sink { [weak self, weak treeView] selectedFolder in
                if self?.delegate.supressnextOpenFolder == true {
                    self?.delegate.supressnextOpenFolder = false
                    return
                }
                
                self?.selectAndReveal(selected: selectedFolder, treeView: treeView)
            }
            .store(in: &cancellables)
            
            homeState.workspaceOutput.$openDoc.sink { [weak self, weak treeView] openDoc in
                if self?.delegate.supressNextOpenDoc == true {
                    self?.delegate.supressNextOpenDoc = false
                    return
                }
                
                guard let openDoc else {
                    return
                }
                
                guard let file = filesModel.idsToFiles[openDoc] else {
                    return
                }
                
                DispatchQueue.main.async {
                    self?.selectAndReveal(selected: openDoc, treeView: treeView)
                }
            }
            .store(in: &cancellables)
            
            filesModel.$files.sink { [weak treeView] _ in
                guard let treeView else { return }
                
                let selectedId = {
                    let ids = treeView.selectedRowIndexes.compactMap { row in
                        (treeView.item(atRow: row) as? File)?.id
                    }
                    
                    return ids.count == 1 ? ids[0] : nil
                }()
                
                treeView.reloadData()
                
                guard let selectedId else { return }
                
                guard let file = filesModel.idsToFiles[selectedId] else { return }
                let row = treeView.row(forItem: file)
                
                treeView.selectRowIndexes([row], byExtendingSelection: false)
            }
            .store(in: &cancellables)
        }
        
        func selectAndReveal(selected: UUID?, treeView: FileTreeOutlineView?) {
            guard let selected else { return }
            guard let treeView else { return }
            
            guard let file = filesModel.idsToFiles[selected] else { return }
            
            self.expandToFile(treeView: treeView, file: file)
            let row = treeView.row(forItem: file)
            treeView.selectRowIndexes([row], byExtendingSelection: false)
            treeView.animator().scrollRowToVisible(row)
        }
        
        func expandToFile(treeView: FileTreeOutlineView, file: File) {
            if let parent = filesModel.idsToFiles[file.parent],
               treeView.row(forItem: file) == -1 {
                if parent.isRoot {
                    return
                }
                
                expandToFile(treeView: treeView, file: parent)
            }
            
            treeView.animator().expandItem(file)
        }
    }
}

#Preview {
    FileTreeView()
        .withCommonPreviewEnvironment()
}

class FileTreeDataSource: NSObject, NSOutlineViewDataSource, NSPasteboardItemDataProvider {
    @ObservedObject var homeState: HomeState
    @ObservedObject var filesModel: FilesViewModel
    
    init(homeState: HomeState, filesModel: FilesViewModel) {
        self.homeState = homeState
        self.filesModel = filesModel
    }
    
    var dragged: [File]? = nil
    
    func outlineView(
            _ outlineView: NSOutlineView,
            numberOfChildrenOfItem item: Any?
    ) -> Int {
        guard let file = item as? File ?? filesModel.root else {
            return 0
        }
        
        let children = filesModel.childrens[file.id] ?? []
        
        return children.count
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            isItemExpandable item: Any
    ) -> Bool {
        let file = item as? File ?? filesModel.root!
        let children = filesModel.childrens[file.id] ?? []
        return file.type == .folder && !children.isEmpty
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            child index: Int,
            ofItem item: Any?
    ) -> Any {
        let file = item as? File ?? filesModel.root!
        let children = filesModel.childrens[file.id] ?? []
        
        return children[index]
    }

    func outlineView(_ outlineView: NSOutlineView, pasteboardWriterForItem item: Any) -> NSPasteboardWriting? {
        let pb = NSPasteboardItem()
        let file = item as! File
        
        pb.setData(try! JSONEncoder().encode(file), forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))
        pb.setDataProvider(self, forTypes: [NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE), .fileURL])

        return pb
    }

    func outlineView(_ outlineView: NSOutlineView, draggingSession session: NSDraggingSession, willBeginAt screenPoint: NSPoint, forItems draggedItems: [Any]) {
        dragged = draggedItems as? [File]
        
        session.draggingPasteboard.setData(try! JSONEncoder().encode(dragged), forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))
    }

    func outlineView(_ outlineView: NSOutlineView, validateDrop info: NSDraggingInfo, proposedItem item: Any?, proposedChildIndex index: Int) -> NSDragOperation {
        let parent = item as? File ?? filesModel.root!
        
        if parent.type == .document {
            return []
        }
        
        if (info.draggingSource as? NSOutlineView) === outlineView {
            return NSDragOperation.move
        } else {
            return NSDragOperation.copy
        }
    }

    func outlineView(_ outlineView: NSOutlineView, acceptDrop info: NSDraggingInfo, item: Any?, childIndex index: Int) -> Bool {
        let parent = item as? File ?? filesModel.root!
        
        if (info.draggingSource as? NSOutlineView) === outlineView {
            moveDraggedFiles(treeView: outlineView, newParent: parent.id)
            outlineView.reloadData()
            return true
        } else {
            guard let urls = info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL] else {
                return false
            }
            
            if(parent.type == .document) {
                return false
            }
                        
            return ImportExportHelper.importFiles(homeState: homeState, filesModel: filesModel, sources: urls.map({ url in url.path(percentEncoded: false)}), destination: parent.id)
        }
    }
    
    func moveDraggedFiles(treeView: NSOutlineView, newParent: UUID) {
        for file in dragged ?? [] {
            if case .failure(let err) = AppState.lb.moveFile(id: file.id, newParent: newParent) {
                AppState.shared.error = .lb(error: err)
                break
            }
        }
    }
    
    func pasteboard(_ pasteboard: NSPasteboard?, item: NSPasteboardItem, provideDataForType type: NSPasteboard.PasteboardType) {
        if(type == .fileURL) {
            let files = try! JSONDecoder().decode([File].self, from: item.data(forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))!)
            
            pasteboard?.clearContents()
            var pasteboardItems: [NSPasteboardItem] = []

            for file in files {
                if let dest = ImportExportHelper.exportFilesToTempDir(homeState: homeState, file: file) {
                    let newItem = NSPasteboardItem()
                    newItem.setData(dest.dataRepresentation, forType: .fileURL)
                    pasteboardItems.append(newItem)

                }
            }
            
            pasteboard?.writeObjects(pasteboardItems)
        }
    }
    
    func outlineView(_ outlineView: NSOutlineView, draggingSession session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) {
        if operation == .move {
            dragged = nil
        }
    }

    static let REORDER_PASTEBOARD_TYPE = "net.lockbook.metadata"
}

protocol MenuOutlineViewDelegate : NSOutlineViewDelegate {
    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu?
}

class FileTreeOutlineView: NSOutlineView {
    init() {
        super.init(frame: .zero)
        target = self
        action = #selector(outlineViewClicked(_:))
    }

    @objc private func outlineViewClicked(_ outlineView: NSOutlineView) {
        
        guard let file = item(atRow: clickedRow) as? File else {
            return
        }
        let delegate = (delegate as! FileTreeDelegate)
        
        delegate.homeState.closeWorkspaceBlockingScreens()
        
        if(file.type == .document) {
            delegate.supressNextOpenDoc = true
        } else {
            delegate.supressnextOpenFolder = true
        }
        
        guard let event = outlineView.window?.currentEvent else {
            return
        }
        
        if event.modifierFlags.contains(.command) || event.modifierFlags.contains(.shift) {
            return
        }
        
        if file.type == .folder {
            if isItemExpanded(file) {
                animator().collapseItem(file)
            } else {
                animator().expandItem(file)
            }
            
            delegate.workspaceInput.selectFolder(id: file.id)
        } else {
            delegate.workspaceInput.selectFolder(id: file.parent)
            delegate.workspaceInput.openFile(id: file.id)
        }
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        let point = self.convert(event.locationInWindow, from: nil)
        let row = self.row(at: point)
        let item = item(atRow: row)

        return (delegate as! MenuOutlineViewDelegate).outlineView(self, menuForItem: item)
    }
    
    override var acceptsFirstResponder: Bool {
        // TODO: Turn to true to enable some form of keyboard navigation
        return false
    }
    
    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

}

class FileTreeDelegate: NSObject, MenuOutlineViewDelegate {
    @ObservedObject var homeState: HomeState
    @ObservedObject var filesModel: FilesViewModel
    @ObservedObject var workspaceInput: WorkspaceInputState
    
    var supressNextOpenDoc = false
    var supressnextOpenFolder = false
    
    init(homeState: HomeState, filesModel: FilesViewModel, workspaceInput: WorkspaceInputState) {
        self.homeState = homeState
        self.filesModel = filesModel
        self.workspaceInput = workspaceInput
    }

    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu? {
        let menu = NSMenu()
        let parent = item as? File ?? filesModel.root!
        
        if outlineView.selectedRowIndexes.count > 1 {
            let consolidatedSelection = filesModel.getConsolidatedSelection()
            
            menu.addItem(DeleteMenuItem(filesModel: filesModel, files: consolidatedSelection))
            menu.addItem(ShareMultipleToMenuItem(homeState: homeState, files: consolidatedSelection, fileTree: outlineView))
            
            return menu
        }
        
        if parent.type == .folder {
            menu.addItem(CreateDocumentMenuItem(workspaceInput: workspaceInput, file: parent))
            menu.addItem(CreateDrawingMenuItem(workspaceInput: workspaceInput, file: parent))
            menu.addItem(CreateFolderMenuItem(homeState: homeState, file: parent))
        }

        if parent.id != parent.parent {
            menu.addItem(RenameFileMenuItem(homeState: homeState, file: parent))
            menu.addItem(ShareMenuItem(homeState: homeState, file: parent))
            menu.addItem(ShareExternallyMenu(homeState: homeState, file: parent, fileTree: outlineView))
            menu.addItem(DeleteMenuItem(filesModel: filesModel, files: [parent]))
        }
        
        return menu
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            viewFor tableColumn: NSTableColumn?,
            item: Any
    ) -> NSView? {
        let file = item as! File
        return FileItemView(file: file)
    }
    
    func outlineViewSelectionIsChanging(_ notification: Notification) {
        supressNextOpenDoc = true
        supressnextOpenFolder = true
    }
    
    func outlineViewSelectionDidChange(_ notification: Notification) {
        guard let outlineView = notification.object as? NSOutlineView else { return }
        
        filesModel.selectedFilesState = .unselected
        
        for selectedRow in outlineView.selectedRowIndexes {
            guard let file = outlineView.item(atRow: selectedRow) as? File else {
                return
            }

            filesModel.addFileToSelection(file: file)
        }
    }
    
    func outlineView(_ outlineView: NSOutlineView, heightOfRowByItem item: Any) -> CGFloat {
        return 23
    }
}

class FileItemView: NSTableCellView {
    init(file: File) {
        super.init(frame: .zero)
        
        let image = NSImage(systemSymbolName: FileIconHelper.fileToSystemImageName(file: file), accessibilityDescription: nil)!
        image.isTemplate = true

        let icon = NSImageView()
        icon.image = image
        icon.translatesAutoresizingMaskIntoConstraints = false
        if file.type == .folder {
            icon.contentTintColor = .controlAccentColor
        } else {
            icon.contentTintColor = .labelColor
        }

        let label = NSTextField(labelWithString: file.name)
        label.isEditable = false
        label.isBezeled = false
        label.drawsBackground = false
        label.lineBreakMode = .byTruncatingTail
        label.setContentHuggingPriority(.defaultHigh, for: .horizontal)
        icon.setContentCompressionResistancePriority(.required, for: .horizontal)

        let stackView = NSStackView(views: [icon, label])
        stackView.orientation = .horizontal
        stackView.spacing = 6
        stackView.translatesAutoresizingMaskIntoConstraints = false

        addSubview(stackView)

        NSLayoutConstraint.activate([
            stackView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 5),
            stackView.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
            stackView.topAnchor.constraint(equalTo: topAnchor),
            stackView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}
