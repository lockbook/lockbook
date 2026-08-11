import SwiftUI

struct WhimsicalArrow: Shape {
    func path(in rect: CGRect) -> Path {
        let w = rect.width
        let h = rect.height

        let start = CGPoint(x: rect.minX + 0.10 * w, y: rect.minY + 0.05 * h)
        let control1 = CGPoint(x: rect.minX + 1.05 * w, y: rect.minY + 0.25 * h)
        let control2 = CGPoint(x: rect.minX + 0.75 * w, y: rect.minY + 0.55 * h)
        let end = CGPoint(x: rect.minX + 0.85 * w, y: rect.minY + 0.97 * h)

        var path = Path()
        path.move(to: start)
        path.addCurve(to: end, control1: control1, control2: control2)

        let angle = atan2(end.y - control2.y, end.x - control2.x)
        let headLength = min(w, h) * 0.22
        for headAngle in [angle + .pi * 5 / 6, angle - .pi * 5 / 6] {
            path.move(to: end)
            path.addLine(to: CGPoint(
                x: end.x + cos(headAngle) * headLength,
                y: end.y + sin(headAngle) * headLength
            ))
        }

        return path
    }
}

struct AnimatedWhimsicalArrow: View {
    let width: CGFloat
    let height: CGFloat
    var rotated = false

    @State private var progress: CGFloat = 0

    var body: some View {
        WhimsicalArrow()
            .trim(from: 0, to: progress)
            .stroke(
                Color.accentColor.opacity(0.7),
                style: StrokeStyle(lineWidth: 2.5, lineCap: .round, lineJoin: .round)
            )
            .frame(width: width, height: height)
            .rotationEffect(.degrees(rotated ? 180 : 0))
            .onAppear {
                progress = 0
                withAnimation(.easeOut(duration: 0.9).delay(0.4)) {
                    progress = 1
                }
            }
    }
}

struct StartHereHint: View {
    let leadingInset: CGFloat

    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            AnimatedWhimsicalArrow(width: 44, height: 64, rotated: true)

            Text("Start here")
                .font(.callout.weight(.medium))
                .foregroundStyle(Color.accentColor)
                .offset(y: 7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.leading, leadingInset)
        .allowsHitTesting(false)
    }
}

struct CreateButtonArrow: View {
    static var alignment: Alignment {
        #if os(iOS)
            .bottomTrailing
        #else
            .topLeading
        #endif
    }

    #if os(iOS)
        @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    #endif

    var body: some View {
        #if os(iOS)
            if horizontalSizeClass == .compact {
                AnimatedWhimsicalArrow(width: 84, height: 140)
                    .padding(.trailing, 40)
                    .allowsHitTesting(false)
            }
        #else
            AnimatedWhimsicalArrow(width: 84, height: 140, rotated: true)
                .padding(.leading, 44)
                .padding(.top, 6)
                .allowsHitTesting(false)
        #endif
    }
}
