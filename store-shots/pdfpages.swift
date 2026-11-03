// pdfpages.swift IN.pdf OUT.png  — contact sheet of every page, 4 per row
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
let a = CommandLine.arguments
let doc = CGPDFDocument(URL(fileURLWithPath: a[1]) as CFURL)!
let n = doc.numberOfPages, cols = 4, w = 300, h = 388
let rows = (n + cols - 1) / cols
let ctx = CGContext(data: nil, width: w * cols, height: h * rows, bitsPerComponent: 8, bytesPerRow: 0,
                    space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setFillColor(red: 1, green: 1, blue: 1, alpha: 1); ctx.fill(CGRect(x: 0, y: 0, width: w * cols, height: h * rows))
for i in 1...n {
    let page = doc.page(at: i)!
    let box = page.getBoxRect(.mediaBox)
    let col = (i - 1) % cols, row = (i - 1) / cols
    ctx.saveGState()
    ctx.translateBy(x: CGFloat(col * w), y: CGFloat((rows - 1 - row) * h))
    ctx.scaleBy(x: CGFloat(w) / box.width, y: CGFloat(h) / box.height)
    ctx.drawPDFPage(page)
    ctx.restoreGState()
}
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[2]) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dest, ctx.makeImage()!, nil); CGImageDestinationFinalize(dest)
