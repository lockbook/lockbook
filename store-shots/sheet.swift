// sheet.swift OUT.png cols IN.png...  — contact sheet, equal tiles
import Foundation; import CoreGraphics; import ImageIO; import UniformTypeIdentifiers
let a = CommandLine.arguments; let cols = Int(a[2])!; let files = Array(a[3...])
let tw = 640, th = 360, gap = 16, rows = (files.count + cols - 1) / cols
let W = cols * tw + (cols + 1) * gap, H = rows * th + (rows + 1) * gap
let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setFillColor(red: 0.12, green: 0.12, blue: 0.13, alpha: 1); ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
ctx.interpolationQuality = .high
for (i, f) in files.enumerated() {
    let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: f) as CFURL, nil)!, img = CGImageSourceCreateImageAtIndex(src, 0, nil)!
    let c = i % cols, r = i / cols
    ctx.draw(img, in: CGRect(x: gap + c * (tw + gap), y: H - (gap + (r + 1) * th + r * gap), width: tw, height: th))
}
let d = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[1]) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(d, ctx.makeImage()!, nil); CGImageDestinationFinalize(d)
