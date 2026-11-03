// phone.swift OUT.png SCREEN.png WIDTH
// Frames a phone screenshot: a near-black bezel with continuous corners,
// the screen clipped inside it, scaled to WIDTH pixels across the bezel.
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

let a = CommandLine.arguments
let out = a[1]; let W = Double(a[3])!
let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[2]) as CFURL, nil)!
let screen = CGImageSourceCreateImageAtIndex(src, 0, nil)!
let bezel = W * 0.028
let radius = W * 0.15
let sw = W - 2 * bezel
let sh = sw * Double(screen.height) / Double(screen.width)
let H = sh + 2 * bezel
let cs = CGColorSpace(name: CGColorSpace.sRGB)!
let ctx = CGContext(data: nil, width: Int(W.rounded()), height: Int(H.rounded()), bitsPerComponent: 8, bytesPerRow: 0, space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setAllowsAntialiasing(true)
ctx.setShouldAntialias(true)
let outer = CGPath(roundedRect: CGRect(x: 0, y: 0, width: W, height: H), cornerWidth: radius, cornerHeight: radius, transform: nil)
ctx.addPath(outer)
ctx.setFillColor(CGColor(red: 0.09, green: 0.09, blue: 0.1, alpha: 1))
ctx.fillPath()
let inner = CGPath(roundedRect: CGRect(x: bezel, y: bezel, width: sw, height: sh), cornerWidth: radius - bezel, cornerHeight: radius - bezel, transform: nil)
ctx.saveGState()
ctx.addPath(inner)
ctx.clip()
ctx.interpolationQuality = .high
ctx.draw(screen, in: CGRect(x: bezel, y: bezel, width: sw, height: sh))
ctx.restoreGState()
let img = ctx.makeImage()!
let dst = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dst, img, nil)
CGImageDestinationFinalize(dst)
print("wrote \(out) \(Int(W))x\(Int(H))")
