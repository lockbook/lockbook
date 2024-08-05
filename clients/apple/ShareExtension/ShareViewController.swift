//
//  ShareViewController.swift
//  ShareExtension
//
//  Created by Smail Barkouch on 7/31/24.
//

import SwiftUI
import UIKit
import UniformTypeIdentifiers

class ShareViewController: UIViewController {

    var sharedItems: [Any] = []
    var processed: [String] = []
    
    override func viewDidLoad() {
        super.viewDidLoad()
        
        guard let extContext = extensionContext else {
            close()
            return
        }
        
        print("the extContext \(extContext.inputItems.count) and then \((extContext.inputItems.first as? NSExtensionItem)?.attachments?.count)")
        
        for input in extContext.inputItems {
            if let input = input as? NSExtensionItem {
                processInputItem(inputItem: input)
            }
        }
        
        if !processed.isEmpty {
            let filePathsQuery = processed.joined(separator: ",")
            guard let url = URL(string: "lb://sharedFiles?\(filePathsQuery.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? "")") else {
                print("early return")
                return
            }

            print("sending over...")
            
            self.extensionContext?.completeRequest(returningItems: nil) { _ in
                self.openURL(url)
            }
        } else {
            print("no PROCESSED")
        }
    }
    
    @objc
    @discardableResult
    func openURL(_ url: URL) -> Bool {
        var responder: UIResponder? = self
        while responder != nil {
            if let application = responder as? UIApplication {
                return application.perform(#selector(openURL(_:)), with: url) != nil
            }
            responder = responder?.next
        }
        return false
    }
    
    func processInputItem(inputItem: NSExtensionItem) {
        for attachment in inputItem.attachments ?? [] {
            processAttachment(attachment: attachment)
        }
    }
    
    func processAttachment(attachment: NSItemProvider) {
        let fileId = UTType.fileURL.identifier
        let imageId = UTType.image.identifier
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.app.lockbook")?.appendingPathComponent("shared").appendingPathComponent(UUID().uuidString) else {
            return
        }
        
        do {
            try FileManager.default.createDirectory(at: containerURL, withIntermediateDirectories: true)
        } catch {
            print("early return 1")
            return
        }
        
        if attachment.hasItemConformingToTypeIdentifier(fileId) {
            let semaphore = DispatchSemaphore(value: 0)

            attachment.loadObject(ofClass: URL.self) { (url, error) in
                guard let url = url else {
                    semaphore.signal()

                    return
                }
                let newHome = containerURL.appendingPathComponent(url.lastPathComponent)
                                
                do {
                    print("copying \(url.absoluteString) to \(newHome)")
                    try FileManager.default.copyItem(at: url as! URL, to: newHome)
                    
                    self.processed.append(newHome.absoluteString)
                } catch {
                    print("Error saving file: \(error)")
                }
                
                semaphore.signal()
            }
            
            semaphore.wait()

        }
//        else if attachment.hasItemConformingToTypeIdentifier(imageId) {
//            attachment.loadObject(ofClass: UIImage.self) { (image, error) in
//                print("got image: \(image)")
//            }
//        }
    }

    func close() {
        self.extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
}
