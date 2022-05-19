import SwiftUI
import SwiftLockbookCore

struct FileTreeView: NSViewRepresentable {
    
    @Binding var currentSelection: DecryptedFileMetadata?
    
    @EnvironmentObject var files: FileService
    
    let scrollView = NSScrollView()
    let treeView = MenuOutlineView()
    let delegate = TreeDelegate()
    var dataSource = DataSource()
    
    func makeNSView(context: Context) -> NSScrollView {
        
        delegate.documentSelected = { currentSelection = $0 }
        
        scrollView.documentView = treeView
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalRuler = true
        scrollView.drawsBackground = false
        
        treeView.autoresizesOutlineColumn = false
        treeView.headerView = nil
        treeView.usesAutomaticRowHeights = true
        treeView.columnAutoresizingStyle = .uniformColumnAutoresizingStyle

        treeView.registerForDraggedTypes([NSPasteboard.PasteboardType(DataSource.REORDER_PASTEBOARD_TYPE)])

        // TODO changing this to true will allow us to accept drops from finder
        treeView.setDraggingSourceOperationMask(NSDragOperation(), forLocal: false)

        treeView.setDraggingSourceOperationMask(NSDragOperation.move, forLocal: true)
        
        let onlyColumn = NSTableColumn()
        onlyColumn.resizingMask = .autoresizingMask
        treeView.addTableColumn(onlyColumn)
        
        treeView.dataSource = dataSource
        treeView.delegate = delegate
        treeView.stronglyReferencesItems = true
        
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        
        return scrollView
    }
    
    func updateNSView(_ nsView: NSScrollView, context: Context) {
        treeView.reloadItem(nil)
    }
}

protocol MenuOutlineViewDelegate : NSOutlineViewDelegate {
    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu?
}

class MenuOutlineView: NSOutlineView {

    override func menu(for event: NSEvent) -> NSMenu? {
        let point = self.convert(event.locationInWindow, from: nil)
        let row = self.row(at: point)
        let item = item(atRow: row)

        return (delegate as! MenuOutlineViewDelegate).outlineView(self, menuForItem: item)
    }

}
