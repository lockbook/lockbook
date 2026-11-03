// ambient.swift OUT.png W H SRC.jpg blur saturation lift grain [lc]
// The shot's own image, scaled to cover, blurred into frosted glass,
// lifted toward white for light mode, with fine grain. A second lift,
// strongest at the top and gone by mid-canvas, is solved so the darkest
// patch of the caption band reaches APCA Lc `lc` (default 90) against
// the headline color #101010.
import Foundation
import CoreGraphics
import CoreImage
import ImageIO
import UniformTypeIdentifiers

let a = CommandLine.arguments
let out = a[1]; let W = Double(a[2])!; let H = Double(a[3])!
let blur = Double(a[5])!, sat = Double(a[6])!, lift = Double(a[7])!, grain = Double(a[8])!
var img = CIImage(contentsOf: URL(fileURLWithPath: a[4]))!
let s = max(W / img.extent.width, H / img.extent.height) * 1.15
img = img.transformed(by: CGAffineTransform(scaleX: s, y: s))
let dx = (img.extent.width - W) / 2, dy = (img.extent.height - H) / 2
img = img.transformed(by: CGAffineTransform(translationX: -img.extent.minX - dx, y: -img.extent.minY - dy))
let extent = CGRect(x: 0, y: 0, width: W, height: H)
img = img.cropped(to: extent)
img = img.applyingFilter("CIColorControls", parameters: [kCIInputSaturationKey: sat, kCIInputBrightnessKey: 0.0, kCIInputContrastKey: 1.0])
img = img.clampedToExtent().applyingFilter("CIGaussianBlur", parameters: [kCIInputRadiusKey: blur]).cropped(to: extent)
// lift toward white: out = img*(1-lift) + white*lift
img = img.applyingFilter("CIColorMatrix", parameters: [
    "inputRVector": CIVector(x: CGFloat(1 - lift), y: 0, z: 0, w: 0),
    "inputGVector": CIVector(x: 0, y: CGFloat(1 - lift), z: 0, w: 0),
    "inputBVector": CIVector(x: 0, y: 0, z: CGFloat(1 - lift), w: 0),
    "inputBiasVector": CIVector(x: CGFloat(lift), y: CGFloat(lift), z: CGFloat(lift), w: 0)])
let targetLc = a.count > 9 ? Double(a[9])! : 90.0
// APCA 0.0.98G: luminance of the headline color, with the black soft clamp.
func apcaY(_ r: Double, _ g: Double, _ b: Double) -> Double {
    func lin(_ c: Double) -> Double { pow(c / 255.0, 2.4) }
    var y = 0.2126729 * lin(r) + 0.7151522 * lin(g) + 0.0721750 * lin(b)
    if y < 0.022 { y += pow(0.022 - y, 1.414) }
    return y
}
func apcaLc(bg: Double, txt: Double) -> Double {
    let sapc = (pow(bg, 0.56) - pow(txt, 0.57)) * 1.14
    return sapc < 0.1 ? 0 : (sapc - 0.027) * 100
}
let textY = apcaY(16, 16, 16)
let ciCtx = CIContext()
let band = CGRect(x: W * 0.1, y: H - 210, width: W * 0.8, height: 120)  // CI is bottom-up
func lifted(_ t: Double) -> CIImage {
    let grad = CIFilter(name: "CILinearGradient", parameters: [
        "inputPoint0": CIVector(x: 0, y: H), "inputColor0": CIColor(red: 1, green: 1, blue: 1, alpha: CGFloat(t)),
        "inputPoint1": CIVector(x: 0, y: H * 0.5), "inputColor1": CIColor(red: 1, green: 1, blue: 1, alpha: 0)])!
        .outputImage!.cropped(to: extent)
    return grad.applyingFilter("CISourceOverCompositing", parameters: [kCIInputBackgroundImageKey: img])
}
func darkestLc(_ image: CIImage) -> Double {
    let cg = ciCtx.createCGImage(image, from: band)!
    let w = cg.width, h = cg.height
    let bctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                         space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    bctx.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
    let px = bctx.data!.assumingMemoryBound(to: UInt8.self)
    var ys: [Double] = []
    var y = 0
    while y < h { var x = 0; while x < w { let i = (y * w + x) * 4
        ys.append(apcaY(Double(px[i]), Double(px[i+1]), Double(px[i+2]))); x += 8 }; y += 8 }
    ys.sort()
    return apcaLc(bg: ys[ys.count / 20], txt: textY)  // 5th percentile: the darkest patch that matters
}
var lo = 0.0, hi = 1.0
let base = darkestLc(img)
if base < targetLc {
    for _ in 0..<12 { let mid = (lo + hi) / 2; if darkestLc(lifted(mid)) >= targetLc { hi = mid } else { lo = mid } }
    img = lifted(hi)
}
print(String(format: "band Lc %.0f -> %.0f with top lift %.2f", base, darkestLc(img), base < targetLc ? hi : 0))
if grain > 0 {
    let noise = CIFilter(name: "CIRandomGenerator")!.outputImage!
        .applyingFilter("CIColorMatrix", parameters: [
            "inputRVector": CIVector(x: 0.33, y: 0.33, z: 0.33, w: 0),
            "inputGVector": CIVector(x: 0.33, y: 0.33, z: 0.33, w: 0),
            "inputBVector": CIVector(x: 0.33, y: 0.33, z: 0.33, w: 0),
            "inputAVector": CIVector(x: 0, y: 0, z: 0, w: CGFloat(grain))])
        .cropped(to: extent)
    img = noise.applyingFilter("CISoftLightBlendMode", parameters: [kCIInputBackgroundImageKey: img])
}
let cg = CIContext().createCGImage(img, from: extent)!
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: out) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(dest, cg, nil)
CGImageDestinationFinalize(dest)
print("wrote \(out)")
