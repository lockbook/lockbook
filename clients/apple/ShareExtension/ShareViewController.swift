import SwiftUI
import UIKit
import UniformTypeIdentifiers
import MobileCoreServices

class ShareViewController: UIViewController {

    var processed: [String] = []
    var failed = false
            
    override func viewDidLoad() {
        super.viewDidLoad()
                
        DispatchQueue.global(qos: .userInitiated).async {
            
            if let sharedFolder = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook")?.appendingPathComponent("shared") {
                if FileManager.default.fileExists(atPath: sharedFolder.path()) {
                    try! FileManager.default.removeItem(at: sharedFolder)
                }
                
                try! FileManager.default.createDirectory(at: sharedFolder, withIntermediateDirectories: true)
                
                self.processEContext(sharedFolder: sharedFolder, eContext: self.extensionContext!)
                
                if self.processed.isEmpty {
                    self.failed = true
                }
                
                if !self.failed {
                    let filePathsQuery = self.processed.joined(separator: ",")
                    let shareURL = URL(string: "lb://sharedFiles?\(filePathsQuery)")!
                    
                    self.dismissDelayed(shareURL)
                    return
                }
            }
            
            self.dismissDelayed()
        }
    }
    
    func processEContext(sharedFolder: URL, eContext: NSExtensionContext) {
        for input in eContext.inputItems {
            if let input = input as? NSExtensionItem {
                for attachment in input.attachments ?? [] {
                    processAttachment(sharedFolder: sharedFolder, attachment: attachment)
                }
            }
        }
        
        if processed.isEmpty {
            self.failed = true
        }
    }
    
    func processAttachment(sharedFolder: URL, attachment: NSItemProvider) {
        let attachmentTypes = [UTType.fileURL.identifier, UTType.image.identifier, UTType.movie.identifier]
        
        for attachmentType in attachmentTypes {
            if attachment.hasItemConformingToTypeIdentifier(attachmentType) {
                let semaphore = DispatchSemaphore(value: 0)

                if attachmentType == UTType.fileURL.identifier {
                    let _ = attachment.loadObject(ofClass: URL.self) { (url, error) in
                        if let url = url {
                            self.importFileIntoAppGroup(sharedFolder: sharedFolder, importing: url)
                        }
                        
                        semaphore.signal()
                    }
                } else {
                    let _ = attachment.loadFileRepresentation(forTypeIdentifier: attachmentType) { (url, error) in
                        if let url = url {
                            self.importFileIntoAppGroup(sharedFolder: sharedFolder, importing: url)
                        }
                        
                        semaphore.signal()
                    }
                }
                
                semaphore.wait()
            }
        }
    }
    
    func importFileIntoAppGroup(sharedFolder: URL, importing: URL) {
        let parent = sharedFolder.appendingPathComponent(UUID().uuidString)
        let newHome = parent.appendingPathComponent(importing.lastPathComponent.removingPercentEncoding!)
        
        do {
            try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: false)
            
            try FileManager.default.copyItem(at: importing, to: newHome)

            self.processed.append(newHome.pathComponents.suffix(3).joined(separator: "/").addingPercentEncoding(withAllowedCharacters: .alphanumerics)!)
        } catch {
            self.failed = true
        }
    }
    
    private func dismissDelayed(_ shareURL: URL? = nil) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25){
            if let shareURL = shareURL {
                self.extensionContext!.completeRequest(returningItems: nil) { _ in
                    self.openURL(shareURL)
                }
            } else {
                self.extensionContext!.completeRequest(returningItems: nil, completionHandler: nil)
            }
        }
    }
    
    @discardableResult
       @objc func openURL(_ url: URL) -> Bool {
           var responder: UIResponder? = self
           while responder != nil {
               if let application = responder as? UIApplication {
                   application.open(url, options: [:]) {_ in }
                   return true
               }
               responder = responder?.next
           }
           return false
       }
}
