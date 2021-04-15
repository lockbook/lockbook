import SwiftUI

struct GridView: UIViewRepresentable {
    func makeUIView(context: Context) -> GridUIView {
        let view = GridUIView()
        view.backgroundColor = .clear
        return view
    }

    func updateUIView(_ view: GridUIView, context: Context) {

    }

}

struct GridView_Previews: PreviewProvider {
    static var previews: some View {
        GridView()
    }
}

class GridUIView: UIView
{
    private var path = UIBezierPath()
    fileprivate var gridWidthMultiple: CGFloat
    {
        return 40
    }
    fileprivate var gridHeightMultiple : CGFloat
    {
        return 80
    }

    fileprivate var gridWidth: CGFloat
    {
        return bounds.width/CGFloat(gridWidthMultiple)
    }

    fileprivate var gridHeight: CGFloat
    {
        return bounds.height/CGFloat(gridHeightMultiple)
    }

    fileprivate var gridCenter: CGPoint {
        return CGPoint(x: bounds.midX, y: bounds.midY)
    }

    fileprivate func drawGrid()
    {
        path = UIBezierPath()
        path.lineWidth = 1.0

        for index in 1...Int(gridWidthMultiple) - 1
        {
            let start = CGPoint(x: CGFloat(index) * gridWidth, y: 0)
            let end = CGPoint(x: CGFloat(index) * gridWidth, y:bounds.height)
            path.move(to: start)
            path.addLine(to: end)
        }

        for index in 1...Int(gridHeightMultiple) - 1 {
            let start = CGPoint(x: 0, y: CGFloat(index) * gridHeight)
            let end = CGPoint(x: bounds.width, y: CGFloat(index) * gridHeight)
            path.move(to: start)
            path.addLine(to: end)
        }

        //Close the path.
        path.close()

    }

    override func draw(_ rect: CGRect)
    {
        drawGrid()

        // Specify a border (stroke) color.
        UIColor(.gray.opacity(0.2)).setStroke()
        path.stroke()
    }
}
