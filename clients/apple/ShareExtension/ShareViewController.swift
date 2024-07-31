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
        
        let contentView = UIHostingController(rootView: ShareExtensionView())
        self.addChild(contentView)
        self.view.addSubview(contentView.view)
        
        // set up constraints
        contentView.view.translatesAutoresizingMaskIntoConstraints = false
        contentView.view.topAnchor.constraint(equalTo: self.view.topAnchor).isActive = true
        contentView.view.bottomAnchor.constraint (equalTo: self.view.bottomAnchor).isActive = true
        contentView.view.leftAnchor.constraint(equalTo: self.view.leftAnchor).isActive = true
        contentView.view.rightAnchor.constraint (equalTo: self.view.rightAnchor).isActive = true
    }
    
    func processInputItem(inputItem: NSExtensionItem) {
        for attachment in inputItem.attachments ?? [] {
            processAttachment(attachment: attachment)
        }
    }
    
    func processAttachment(attachment: NSItemProvider) {
        let fileId = UTType.fileURL.identifier
        let imageId = UTType.image.identifier
        
        if attachment.hasItemConformingToTypeIdentifier(fileId) {
            attachment.loadObject(ofClass: URL.self) { (url, error) in
                print("got URL: \(url?.absoluteString)")
            }
        } else if attachment.hasItemConformingToTypeIdentifier(imageId) {
            attachment.loadObject(ofClass: UIImage.self) { (image, error) in
                print("got image: \(image)")
            }
        }
    }

    func close() {
        self.extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
}
