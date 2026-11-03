// backdrop.swift OUT.png W H base_hex blur grain [hex cx cy rx ry alpha]...
// Soft color fields on a flat base, blurred, with fine monochrome grain.
// cx cy rx ry are fractions of the canvas; blur in px; grain 0..1.
import Foundation
import CoreGraphics
import CoreImage
import ImageIO
import UniformTypeIdentifiers

let a = CommandLine.arguments
let out = a[1]; let W = Int(a[2])!; let H = Int(a[3])!
func rgb(_ h: String) -> (CGFloat, CGFloat, CGFloat) {
    let v = UInt32(h.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16)!
    return (CGFloat((v >> 16) & 255) / 255, CGFloat((v >> 8) & 255) / 255, CGFloat(v & 255) / 255)
}
let blur = Double(a[5])!, grain = Double(a[6])!
let cs = CGColorSpaceCreateDeviceRGB()
let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0, space: cs,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
let (br, bg, bb) = rgb(a[4])
ctx.setFillColor(red: br, green: bg, blue: bb, alpha: 1)
ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
var i = 7
while i + 5 < a.count {
    let (r, g, b) = rgb(a[i])
    let cx = Double(a[i+1])! * Double(W), cy = (1 - Double(a[i+2])!) * Double(H)
    let rx = Double(a[i+3])! * Double(W), ry = Double(a[i+4])! * Double(H)
    let al = CGFloat(Double(a[i+5])!)
    let colors = [CGColor(red: r, green: g, blue: b, alpha: al), CGColor(red: r, green: g, blue: b, alpha: 0)] as CFArray
    let grad = CGGradient(colorsSpace: cs, colors: colors, locations: [0, 1])!
    ctx.saveGState()
    ctx.translateBy(x: cx, y: cy); ctx.scaleBy(x: rx, y: ry)
    ctx.drawRadialGradient(grad, startCenter: .zero, startRadius: 0, endCenter: .zero, endRadius: 1, options: [])
    ctx.restoreGState()
    i += 6
}
var img = CIImage(cgImage: ctx.makeImage()!)
let extent = img.extent
if blur > 0 {
    img = img.clampedToExtent().applyingFilter("CIGaussianBlur", parameters: [kCIInputRadiusKey: blur]).cropped(to: extent)
}
if grain > 0 {
    let noise = CIFilter(name: "CIRandomGenerator")!.outputImage!
        .applyingFilter("CIColorMatrix", parameters: [
            "inputRVector": CIVector(x: 0.33, y: 0.33, z: 0.33, w: 0),
            "inputGVector": CIVector(x: 0.33, y: 0.33, z: 0.33, w: 0),
            "inputBVector": CIVector(x: 0.33, y: 0.33, z: 0.33, w: 0),
            "inputAVector": CIVector(x: 0, y: 0, z: 0, w: CGFloat(grain)),
            "inputBiasVector": CIVector(x: -0.5 * CGFloat(grain) * 0 , y: 0, z: 0, w: 0)])
        .cropped(to: extent)
    img = noise.applyingFilter("CISoftLightBlendMode", parameters: [kCIInputBackgroundImageKey: img])
}
let cictx = CIContext()
let cg = cictx.createCGImage(img, from: extent)!
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dest, cg, nil)
CGImageDestinationFinalize(dest)
print("wrote \(out)")
