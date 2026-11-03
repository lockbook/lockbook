// slate.swift IN.png OUT.png FRAC [GRAY]
// Fades a flat slate (GRAY 0-1, default 0.5) up from the bottom edge over
// FRAC of the height, so the backdrop grounds in gray without a hard line.
import Foundation
import CoreImage
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
let a = CommandLine.arguments
let src = CIImage(contentsOf: URL(fileURLWithPath: a[1]))!
let frac = Double(a[3])!, gray = a.count > 4 ? Double(a[4])! : 0.5
let H = src.extent.height
let grad = CIFilter(name: "CILinearGradient", parameters: [
    "inputPoint0": CIVector(x: 0, y: 0), "inputColor0": CIColor(red: gray, green: gray, blue: gray, alpha: 1),
    "inputPoint1": CIVector(x: 0, y: H * frac), "inputColor1": CIColor(red: gray, green: gray, blue: gray, alpha: 0)])!.outputImage!.cropped(to: src.extent)
let out = grad.composited(over: src)
let ctx = CIContext()
let cg = ctx.createCGImage(out, from: src.extent)!
let d = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[2]) as CFURL, UTType.png.identifier as CFString, 1, nil)!
CGImageDestinationAddImage(d, cg, nil); CGImageDestinationFinalize(d)
print("wrote \(a[2])")
