import SwiftUI

struct UsageIndicator: View {
    let numerator: UInt64
    @State var realNumerator: UInt64 = 0
    let denominator: UInt64
    let suffix: String
    
    var body: some View {
        VStack {
            ZStack {
                RoundedRectangle(cornerRadius: 20)
                    .stroke(style: StrokeStyle(lineWidth: 20.0, lineCap: .round, lineJoin: .round))
                    .opacity(0.3)
                RoundedRectangle(cornerRadius: 20)
                    .scale(x: min((CGFloat(realNumerator)/CGFloat(denominator)), 1.0), y: 1.0, anchor: .leading)
                    .stroke(style: StrokeStyle(lineWidth: 20.0, lineCap: .round, lineJoin: .round))
            }
            .frame(width: 300, height: 1, alignment: .center)
            .padding(.vertical)
            Text("\(numerator) \(suffix)")
        }
        .padding()
        .onAppear {
            withAnimation(.linear) {
                realNumerator = numerator
            }
        }
    }
}

struct UsageIndicator_Previews: PreviewProvider {
    static var previews: some View {
        UsageIndicator(numerator: 4, denominator: 10, suffix: "B")
            .previewLayout(.fixed(width: 400, height: 400))
    }
}
