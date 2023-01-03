import SwiftUI

struct ColorProgressBar: View {
    let value: Double
    
    var color: Color {
        if value < 0.8 {
            return Color.blue
        }
        
        if value < 0.9 {
            return Color.orange
        }
        
        return Color.red
    }
    
    var body: some View {
        ProgressView(value: value)
            .tint(color)
    }
}

struct Level1: PreviewProvider {
    static var previews: some View {
        ColorProgressBar(value: 0.5)
    }
}

struct Level2: PreviewProvider {
    static var previews: some View {
        ColorProgressBar(value: 0.85)
    }
}

struct Level3: PreviewProvider {
    static var previews: some View {
        ColorProgressBar(value: 0.95)
    }
}
