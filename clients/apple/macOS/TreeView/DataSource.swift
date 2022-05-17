import AppKit
import SwiftLockbookCore

class DataSource: NSObject, NSOutlineViewDataSource {

    func outlineView(
        _ outlineView: NSOutlineView,
        numberOfChildrenOfItem item: Any?
    ) -> Int {
        print("numberOfChildrenOfItem")
        let file = item == nil ? DI.files.root! : item as! DecryptedFileMetadata
        return DI.files.files.filter { $0.parent == file.id }.count
    }
    
    func outlineView(
        _ outlineView: NSOutlineView,
        isItemExpandable item: Any
    ) -> Bool {

        let file = item as! DecryptedFileMetadata
        print(file.fileType == .Folder
              && !DI.files.files.filter { $0.parent == file.id }.isEmpty)
        
        return file.fileType == .Folder
        && !DI.files.files.filter { $0.parent == file.id }.isEmpty
    }
    
    func outlineView(
        _ outlineView: NSOutlineView,
        child index: Int,
        ofItem item: Any?
    ) -> Any {
        print("child ofItem")
        let parent = item == nil ? DI.files.root! : item as! DecryptedFileMetadata
        let siblings = DI.files.files.filter { $0.parent == parent.id }
        let node = siblings[index]
        return node
    }
}

class TreeDelegate: NSObject, NSOutlineViewDelegate {
    
    var documentSelected: (DecryptedFileMetadata) -> Void = { _ in }
    
    func outlineView(
        _ outlineView: NSOutlineView,
        viewFor tableColumn: NSTableColumn?,
        item: Any
    ) -> NSView? {
        print("viewFor")
        let file = item as! DecryptedFileMetadata
        return FileItemView(file: file)
    }

    func outlineViewItemDidExpand(_ notification: Notification) {
        print("outlineViewItemDidExpand")
    }
    
    func outlineView(_ outlineView: NSOutlineView,
                     shouldSelectItem item: Any) -> Bool {
        let file = item as! DecryptedFileMetadata
        return file.fileType == .Document
    }
    
    func outlineViewSelectionDidChange(_ notification: Notification) {
        let outlineView = notification.object as! NSOutlineView
        if outlineView.selectedRow != -1 {
            let file = outlineView.item(atRow: outlineView.selectedRow) as! DecryptedFileMetadata
            documentSelected(file)
        }
    }

}


