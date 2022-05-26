import AppKit
import SwiftLockbookCore

// Reference for drag and drop:
// https://github.com/KinematicSystems/NSOutlineViewReorder/blob/master/OutlineViewReorder/OutlineDataSource.swift
class DataSource: NSObject, NSOutlineViewDataSource, NSPasteboardItemDataProvider {

    var dragged: DecryptedFileMetadata? = nil

    func outlineView(
            _ outlineView: NSOutlineView,
            numberOfChildrenOfItem item: Any?
    ) -> Int {
        let file = item as? DecryptedFileMetadata
        let children = DI.files.childrenOf(file)
        print(children.count)
        return children.count
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            isItemExpandable item: Any
    ) -> Bool {
        let file = item as! DecryptedFileMetadata

        return file.fileType == .Folder
                && !DI.files.childrenOf(file).isEmpty
    }

    func outlineView(
            _ outlineView: NSOutlineView,
            child index: Int,
            ofItem item: Any?
    ) -> Any {
        let parent = item as? DecryptedFileMetadata
        let siblings = DI.files.childrenOf(parent)
        let node = siblings[index]
        return node
    }

    func outlineView(_ outlineView: NSOutlineView,
                     pasteboardWriterForItem item: Any) -> NSPasteboardWriting? {
        let pb = NSPasteboardItem()
        pb.setDataProvider(self, forTypes: [NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE)])

        return pb
    }

    func outlineView(_ outlineView: NSOutlineView, draggingSession session: NSDraggingSession, willBeginAt screenPoint: NSPoint, forItems draggedItems: [Any]) {
        dragged = draggedItems[0] as? DecryptedFileMetadata
        session.draggingPasteboard.setData(Data(), forType: NSPasteboard.PasteboardType(Self.REORDER_PASTEBOARD_TYPE))
    }

    func outlineView(_ outlineView: NSOutlineView, validateDrop info: NSDraggingInfo, proposedItem item: Any?, proposedChildIndex index: Int) -> NSDragOperation {
        let parent = item == nil ? DI.files.root! : item as! DecryptedFileMetadata
        if parent.fileType == .Document {
            return []
        }
        return NSDragOperation.move
    }

    func outlineView(_ outlineView: NSOutlineView, acceptDrop info: NSDraggingInfo, item: Any?, childIndex index: Int) -> Bool {
        let parent = item == nil ? DI.files.root! : item as! DecryptedFileMetadata
        return DI.files.moveFileSync(id: dragged!.id, newParent: parent.id)
    }

    // never called
    func pasteboard(_ pasteboard: NSPasteboard?, item: NSPasteboardItem, provideDataForType type: NSPasteboard.PasteboardType) {
        print("pasteboard inspected")
        let s = "Outline Pasteboard Item"
        item.setString(s, forType: type)
    }

    func outlineView(_ outlineView: NSOutlineView, draggingSession session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation) {
        dragged = nil
    }

    static let REORDER_PASTEBOARD_TYPE = "net.lockbook.metadata"
}

class TreeDelegate: NSObject, MenuOutlineViewDelegate {
    var documentSelected: (DecryptedFileMetadata) -> Void = { _ in
    }

    func outlineView(_ outlineView: NSOutlineView, menuForItem item: Any?) -> NSMenu? {
        let menu = NSMenu()
        let parent = item == nil ? DI.files.root! : item as! DecryptedFileMetadata

        if parent.fileType == .Folder {
            menu.addItem(Create(file: parent))
        }

        if parent.id != parent.parent {
            menu.addItem(Rename(file: parent))
            menu.addItem(Delete(file: parent))
        }
        return menu
    }


    func outlineView(
            _ outlineView: NSOutlineView,
            viewFor tableColumn: NSTableColumn?,
            item: Any
    ) -> NSView? {
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


