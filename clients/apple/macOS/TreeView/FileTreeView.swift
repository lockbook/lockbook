import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore

struct FileTreeView: NSViewRepresentable, Equatable {
    let scrollView = NSScrollView()
    let treeView = MenuOutlineView()
    let delegate = TreeDelegate()
    let dataSource = DataSource()

    @EnvironmentObject var files: FileService
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
                
//        selectOpenDoc()
    }
        
    func selectOpenDoc() {
        if workspace.openDoc == nil && dataSource.selectedDoc != nil {
            dataSource.selectedDoc = nil
            treeView.selectRowIndexes(IndexSet(), byExtendingSelection: false)
        } else if let openDoc = workspace.openDoc,
            let meta = files.idsAndFiles[openDoc] {
                        
            if workspace.openDoc != nil && dataSource.selectedDoc != workspace.openDoc {
                expandToFile(meta: meta)
                dataSource.selectedDoc = workspace.openDoc
                
                treeView.selectRowIndexes(IndexSet(integer: treeView.row(forItem: meta)), byExtendingSelection: true)
                treeView.animator().scrollRowToVisible(treeView.row(forItem: meta))
            } else {
                treeView.selectRowIndexes(IndexSet(integer: treeView.row(forItem: meta)), byExtendingSelection: true)
            }
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
            if meta.fileType == .Document {
                DI.workspace.requestOpenDoc(meta.id)
                
                return
            }
            
            if isItemExpanded(meta) {
                DI.files.expandedFolders.removeAll(where: { $0 == meta.id })
                animator().collapseItem(meta)
            } else {
                DI.files.expandedFolders.append(meta.id)
                animator().expandItem(meta)
            }
            
            DI.workspace.selectedFolder = meta.id
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
