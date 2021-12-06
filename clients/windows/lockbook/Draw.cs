using Core;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using Windows.Foundation;
using Windows.UI;
using Windows.UI.Input.Inking;

namespace Lockbook {
    public class DrawingContext {
        public List<Windows.UI.Input.Inking.InkStroke> strokes;
        public Dictionary<Windows.UI.Input.Inking.InkStroke, List<Windows.UI.Input.Inking.InkStroke>> splitStrokes;
        public Dictionary<ColorAlias, ColorRGB> theme;

        public DrawingContext() {
            strokes = new List<Windows.UI.Input.Inking.InkStroke>();
            splitStrokes = new Dictionary<Windows.UI.Input.Inking.InkStroke, List<Windows.UI.Input.Inking.InkStroke>>(new Draw.InkStrokeComparer());
        }
    }

    public static class Draw {
        private static InkStrokeBuilder inkStrokeBuilder { get; } = new InkStrokeBuilder();

        public static DrawingContext CoreContextToContext(Core.DrawingContext coreContext) {
            return new DrawingContext {
                strokes = coreContext.strokes.Select(CoreInkStrokeToInkStroke).ToList(),
                splitStrokes = coreContext.splitStrokes.ToDictionary(
                    kvp => CoreInkStrokeToInkStroke(kvp.Key),
                    kvp => kvp.Value.Select(CoreInkStrokeToInkStroke).ToList(),
                    new InkStrokeComparer()),
                theme = coreContext.theme,
            };
        }

        public static Core.DrawingContext ContextToCoreContext(DrawingContext context) {
            return new Core.DrawingContext {
                strokes = context.strokes.Select(InkStrokeToCoreInkStroke).ToList(),
                splitStrokes = context.splitStrokes.ToDictionary(
                    kvp => InkStrokeToCoreInkStroke(kvp.Key),
                    kvp => kvp.Value.Select(InkStrokeToCoreInkStroke).ToList(),
                    new Core.Draw.InkStrokeComparer()),
                theme = context.theme,
            };
        }

        private static Windows.UI.Input.Inking.InkStroke CoreInkStrokeToInkStroke(Core.InkStroke stroke) {
            var attributes = new InkDrawingAttributes {
                Size = new Size(stroke.size, stroke.size),
                Color = Color.FromArgb(stroke.alpha, stroke.color.r, stroke.color.g, stroke.color.b),
            };
            inkStrokeBuilder.SetDefaultDrawingAttributes(attributes);
            return inkStrokeBuilder.CreateStrokeFromInkPoints(stroke.points.Select(point => new Windows.UI.Input.Inking.InkPoint(new Point(point.x, point.y), point.pressure, 0, 0, 0)), Matrix3x2.Identity);
        }

        private static Core.InkStroke InkStrokeToCoreInkStroke(Windows.UI.Input.Inking.InkStroke stroke) {
            return new Core.InkStroke {
                size = (float)stroke.DrawingAttributes.Size.Width,
                points = stroke.GetInkPoints().Select(point => new Core.InkPoint {
                    x = (float)point.Position.X,
                    y = (float)point.Position.Y,
                    pressure = point.Pressure
                }).ToList(),
                color = new ColorRGB {
                    r = stroke.DrawingAttributes.Color.R,
                    b = stroke.DrawingAttributes.Color.B,
                    g = stroke.DrawingAttributes.Color.G,
                },
                alpha = stroke.DrawingAttributes.Color.A,
            };
        }

        public static Color GetUIColor(this Dictionary<ColorAlias, ColorRGB> theme, ColorAlias color, float alpha) {
            if (theme == null || theme.Count == 0) {
                return Core.Draw.defaultTheme.GetUIColor(color, alpha);
            }
            var alphaByte = (byte)(alpha * 255);
            var colorRGB = theme[color];
            return Color.FromArgb(alphaByte, colorRGB.r, colorRGB.g, colorRGB.b);
        }

        public static ColorAlias GetColorAlias(this Dictionary<ColorAlias, ColorRGB> theme, Color color) {
            if (theme == null || theme.Count == 0) {
                return Core.Draw.defaultTheme.GetColorAlias(color);
            }
            var inverted = theme.Invert();
            return inverted[new ColorRGB { r = color.R, b = color.B, g = color.G }];
        }

        public class InkStrokeComparer : IEqualityComparer<Windows.UI.Input.Inking.InkStroke> {
            public bool Equals(Windows.UI.Input.Inking.InkStroke x, Windows.UI.Input.Inking.InkStroke y) {
                return new Core.Draw.InkStrokeComparer().Equals(InkStrokeToCoreInkStroke(x), InkStrokeToCoreInkStroke(y));
            }

            public int GetHashCode(Windows.UI.Input.Inking.InkStroke stroke) {
                return new Core.Draw.InkStrokeComparer().GetHashCode(InkStrokeToCoreInkStroke(stroke));
            }
        }
    }
}
