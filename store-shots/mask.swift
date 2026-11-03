// mask.swift IN.png OUT.png x0 y0 x1 y1 keep gap
// Finds words (runs of dark pixels) inside the given region of a render and
// covers every word after the first `keep` with a white pill, so a key phrase
// reads as a secret without inventing fake words.
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

let a = CommandLine.arguments
let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[1]) as CFURL, nil)!
let img = CGImageSourceCreateImageAtIndex(src, 0, nil)!
let x0 = Int(a[3])!, y0 = Int(a[4])!, x1 = Int(a[5])!, y1 = Int(a[6])!
let keep = Int(a[7])!, gap = Int(a[8])!
let W = img.width, H = img.height
let cs = CGColorSpaceCreateDeviceRGB()
let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: W * 4, space: cs,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(img, in: CGRect(x: 0, y: 0, width: W, height: H))
let px = ctx.data!.assumingMemoryBound(to: UInt8.self)
// Bitmap memory runs top-down; only the drawing coordinates are bottom-up.
func dark(_ x: Int, _ y: Int) -> Bool {
    let i = (y * W + x) * 4
    return Int(px[i]) + Int(px[i+1]) + Int(px[i+2]) < 3 * 140
}
// text rows: runs of y with any dark pixel in the region
var rows: [(Int, Int)] = []
var y = y0
while y < y1 {
    var has = false
    for x in x0..<x1 where dark(x, y) { has = true; break }
    if has {
        let start = y
        while y < y1 && (x0..<x1).contains(where: { dark($0, y) }) { y += 1 }
        rows.append((start, y))
    } else { y += 1 }
}
var words: [CGRect] = []
for (ry0, ry1) in rows {
    var x = x0
    var runStart: Int? = nil
    var lastDark = -1000
    while x < x1 {
        let col = (ry0..<ry1).contains(where: { dark(x, $0) })
        if col {
            if runStart == nil { runStart = x }
            lastDark = x
        } else if let s = runStart, x - lastDark > gap {
            words.append(CGRect(x: s, y: ry0, width: lastDark - s + 1, height: ry1 - ry0))
            runStart = nil
        }
        x += 1
    }
    if let s = runStart { words.append(CGRect(x: s, y: ry0, width: lastDark - s + 1, height: ry1 - ry0)) }
}
print("rows \(rows.count) words \(words.count)")
ctx.setFillColor(red: 1, green: 1, blue: 1, alpha: 1)
let padX = CGFloat(rows.first.map { $0.1 - $0.0 } ?? 10) * 0.3
let padY = padX * 0.35
for (i, w) in words.enumerated() where i >= keep {
    let r = CGRect(x: w.minX - padX, y: CGFloat(H) - w.maxY - padY, width: w.width + 2 * padX, height: w.height + 2 * padY)
    ctx.addPath(CGPath(roundedRect: r, cornerWidth: r.height / 2, cornerHeight: r.height / 2, transform: nil))
    ctx.fillPath()
}
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[2]) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
CGImageDestinationFinalize(dest)
print("wrote \(a[2])")
