// compose.swift OUT.png W H bg SRC.png x y scale [SRC2.png x y scale ...] [-- "caption" size x y]
// Places raw app renders on a canvas; a window may run off any edge.
// A caption is one headline; a *starred* run is set in the accent on its tint,
// the way lockbook.net's hero sets its word. CAPTION_FONT names a .ttf to use.
import Foundation
import CoreGraphics
import ImageIO
import CoreText
import UniformTypeIdentifiers

let a = CommandLine.arguments
let out = a[1]; let W = Int(a[2])!; let H = Int(a[3])!
func rgb(_ h: String) -> (CGFloat, CGFloat, CGFloat) {
    let v = UInt32(h.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16)!
    return (CGFloat((v >> 16) & 255) / 255, CGFloat((v >> 8) & 255) / 255, CGFloat(v & 255) / 255)
}
let cs = CGColorSpaceCreateDeviceRGB()
let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0, space: cs,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
if a[4].hasSuffix(".png") {
    // A rendered backdrop instead of a flat color.
    let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[4]) as CFURL, nil)!
    let img = CGImageSourceCreateImageAtIndex(src, 0, nil)!
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: W, height: H))
} else {
    let (r, g, b) = rgb(a[4])
    ctx.setFillColor(red: r, green: g, blue: b, alpha: 1)
    ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
}
// The backdrop darkens slightly toward the bottom edge, so a window that
// bleeds off it reads as sinking into the frame rather than being cut.
let grad = CGGradient(colorsSpace: cs, colors: [
    CGColor(red: 0.08, green: 0.1, blue: 0.14, alpha: 0.0),
    CGColor(red: 0.08, green: 0.1, blue: 0.14, alpha: 0.10)] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: Double(H) * 0.5), end: CGPoint(x: 0, y: 0), options: [])
ctx.interpolationQuality = .high
var i = 5
while i + 3 < a.count && a[i] != "--" {
    let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[i]) as CFURL, nil)!
    let img = CGImageSourceCreateImageAtIndex(src, 0, nil)!
    let x = Double(a[i+1])!, y = Double(a[i+2])!, s = Double(a[i+3])!
    let w = Double(img.width) * s, h = Double(img.height) * s
    let rect = CGRect(x: x, y: Double(H) - y - h, width: w, height: h)
    // A wide, soft shadow lifts the window off the backdrop without a stroke.
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -24), blur: 90,
                  color: CGColor(red: 0.08, green: 0.1, blue: 0.14, alpha: 0.22))
    // the shadow follows the image's alpha, so a framed phone keeps its corners
    ctx.draw(img, in: rect)
    ctx.restoreGState()
    i += 4
}
if i < a.count && a[i] == "--" && i + 4 < a.count {
    let text = a[i+1], size = CGFloat(Double(a[i+2])!), cx = Double(a[i+3])!, cy = Double(a[i+4])!
    var font = CTFontCreateUIFontForLanguage(.system, size, nil)!
    if let file = ProcessInfo.processInfo.environment["CAPTION_FONT"],
       let desc = CTFontManagerCreateFontDescriptorsFromURL(URL(fileURLWithPath: file) as CFURL) as? [CTFontDescriptor], let d = desc.first {
        font = CTFontCreateWithFontDescriptor(d, size, nil)
    } else {
        font = CTFontCreateCopyWithSymbolicTraits(font, size, nil, .boldTrait, .boldTrait) ?? font
    }
    let fg = CGColor(red: 0x10/255.0, green: 0x10/255.0, blue: 0x10/255.0, alpha: 1)
    let accent = CGColor(red: 0x20/255.0, green: 0x7F/255.0, blue: 0xDF/255.0, alpha: 1)
    let attr = NSMutableAttributedString()
    var accentRanges: [NSRange] = []
    for (k, part) in text.components(separatedBy: "*").enumerated() {
        let start = attr.length
        attr.append(NSAttributedString(string: part, attributes: [
            kCTFontAttributeName as NSAttributedString.Key: font,
            kCTForegroundColorAttributeName as NSAttributedString.Key: k % 2 == 1 ? accent : fg,
            kCTKernAttributeName as NSAttributedString.Key: -size * 0.02]))
        if k % 2 == 1 { accentRanges.append(NSRange(location: start, length: part.count)) }
    }
    let line = CTLineCreateWithAttributedString(attr)
    var ascent: CGFloat = 0, descent: CGFloat = 0
    let width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
    let origin = CGPoint(x: cx - width / 2, y: Double(H) - cy - Double(ascent - descent) / 2)
    for r in accentRanges {
        let x0 = CTLineGetOffsetForStringIndex(line, r.location, nil)
        let x1 = CTLineGetOffsetForStringIndex(line, r.location + r.length, nil)
        let pad = size * 0.12
        let rect = CGRect(x: origin.x + x0 - pad, y: origin.y - descent * 0.9, width: x1 - x0 + 2 * pad, height: ascent * 0.98 + descent * 0.9)
        ctx.setFillColor(CGColor(red: 0x20/255.0, green: 0x7F/255.0, blue: 0xDF/255.0, alpha: 0.12))
        ctx.addPath(CGPath(roundedRect: rect, cornerWidth: size * 0.12, cornerHeight: size * 0.12, transform: nil))
        ctx.fillPath()
    }
    ctx.textPosition = origin
    CTLineDraw(line, ctx)
}
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
CGImageDestinationFinalize(dest)
print("wrote \(out)")
