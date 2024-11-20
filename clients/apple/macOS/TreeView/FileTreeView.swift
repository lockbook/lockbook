import SwiftUI
import SwiftWorkspace

struct FileTreeView: NSViewRepresentable, Equatable {
    let scrollView = NSScrollView()
    let treeView = MenuOutlineView()
    let delegate = TreeDelegate()
    let dataSource = DataSource()

    @EnvironmentObject var files: FileService
    @EnvironmentObject var selected: SelectedFilesState
    @EnvironmentObject var workspace: WorkspaceState
        
    func makeNSView(context: Context) -> NSScrollView {
        scrollView.documentView = treeView
        scrollView.hasVerticalScroller = true
        scrollView.horizontalScrollElasticity = .none
        scrollView.hasHorizontalScroller = false
        scrollView.hasHorizontalRuler = false
        scrollView.drawsBackground = false
        
        treeView.autoresizesOutlineColumn = true
        treeView.headerView = nil
        treeView.usesAutomaticRowHeights = true
        treeView.allowsMultipleSelection = true
        treeView.allowsEmptySelection = true
        
        treeView.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        
        treeView.setDraggingSourceOperationMask(.copy, forLocal: false)
        treeView.setDraggingSourceOperationMask(.move, forLocal: true)
        
        treeView.registerForDraggedTypes([NSPasteboard.PasteboardType(DataSource.REORDER_PASTEBOARD_TYPE), .fileURL])
        
        let onlyColumn = NSTableColumn()
        onlyColumn.resizingMask = .autoresizingMask
        onlyColumn.minWidth = 100
        treeView.addTableColumn(onlyColumn)
        
        treeView.dataSource = dataSource
        treeView.delegate = delegate
        treeView.stronglyReferencesItems = true
                        
        for id in DI.files.expandedFolders {
            if let meta = DI.files.idsAndFiles[id] {
                expandToFile(meta: meta)
            }
        }
        
        selectOpenDoc()

        return scrollView
    }
        
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        if dataSource.lastFilesHash != files.idsAndFiles.hashValue {
            dataSource.lastFilesHash = files.idsAndFiles.hashValue
            
            treeView.reloadData()
        }
        
        for selected in selected.selectedFiles ?? [] {
            let row = treeView.row(forItem: selected)
            if row != -1 && !treeView.isRowSelected(row) {
                treeView.selectRowIndexes([row], byExtendingSelection: true)
            }
        }
                
        selectOpenDoc()
    }
        
    func selectOpenDoc() {
        if let openDocId = workspace.openDoc,
           let meta = files.idsAndFiles[openDocId],
           !treeView.isRowSelected(treeView.row(forItem: meta)),
           dataSource.selectedDoc != openDocId && workspace.openDocRequested == nil {
            
            dataSource.selectedDoc = workspace.openDoc
            selected.selectedFiles = []
            
            expandToFile(meta: meta)
            treeView.selectRowIndexes(IndexSet(integer: treeView.row(forItem: meta)), byExtendingSelection: false)
            treeView.animator().scrollRowToVisible(treeView.row(forItem: meta))
        } else if workspace.openDoc == nil && workspace.openDocRequested == nil && dataSource.selectedDoc != nil {
            dataSource.selectedDoc = nil
            selected.selectedFiles = []
            
            treeView.deselectAll(nil)
        }
    }
    
    func expandToFile(meta: File) {
        if let parentMeta = DI.files.idsAndFiles[meta.parent],
           treeView.row(forItem: meta) == -1 {
            expandToFile(meta: parentMeta)
        }
        
        treeView.animator().expandItem(meta)
    }
    
    static func == (lhs: FileTreeView, rhs: FileTreeView) -> Bool {
        true
    }
}

protocol MenuOutlineViewDelegate : NSOutlineViewDelegate {
    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu?
}

class MenuOutlineView: NSOutlineView {

    init() {
        super.init(frame: .zero)
        target = self
        action = #selector(outlineViewClicked(_:))
    }

    @objc private func outlineViewClicked(_ outlineView: NSOutlineView) {
        if let meta = item(atRow: clickedRow) as? File {
            becomeFirstResponder()
            
            if NSEvent.modifierFlags.contains(.command) {
                DI.selected.addFileToSelection(file: meta)
                
                if let openDocId = DI.workspace.openDoc,
                   let meta = DI.files.idsAndFiles[openDocId],
                   DI.selected.selectedFiles?.contains(meta) == false,
                   outlineView.isRowSelected(outlineView.row(forItem: meta)) {
                    DI.selected.addFileToSelection(file: meta)
                }
                
                return
            } else {
                DI.selected.selectedFiles = []
                for row in outlineView.selectedRowIndexes {
                    if row != clickedRow {
                        outlineView.deselectRow(row)
                    }
                }
            }
            
            if meta.type == .document && DI.workspace.openDoc != meta.id {
                (outlineView.dataSource as! DataSource).selectedDoc = meta.id
                DI.workspace.openDoc = meta.id
                
                outlineView.selectRowIndexes(IndexSet(integer: outlineView.row(forItem: meta)), byExtendingSelection: false)

                DI.workspace.requestOpenDoc(meta.id)
                
                return
            }
            
            DI.selected.selectedFiles = []
            DI.selected.addFileToSelection(file: meta)
            
            if isItemExpanded(meta) {
                DI.files.expandedFolders.removeAll(where: { $0 == meta.id })
                animator().collapseItem(meta)
            } else {
                DI.files.expandedFolders.append(meta.id)
                animator().expandItem(meta)
            }
            
            if meta.type == .folder {
                DI.workspace.selectedFolder = meta.id
            }
        }
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        let point = self.convert(event.locationInWindow, from: nil)
        let row = self.row(at: point)
        let item = item(atRow: row)

        return (delegate as! MenuOutlineViewDelegate).outlineView(self, menuForItem: item)
    }
    
    override var acceptsFirstResponder: Bool {
        return false
    }

    required init(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

}
