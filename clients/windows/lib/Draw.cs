using Core;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using Windows.Foundation;
using Windows.UI;
using Windows.UI.Input.Inking;

namespace lib {
    public class DrawingContext {
        // strokes in the drawing; render these to the canvas
        public List<InkStroke> strokes;
        // windows has a minimum pressure of ~0.16 and maximum pressure of ~1.00
        // this makes it impossible to render strokes with max girth > 6 * min girth (henceforth 'HDR strokes')
        // to solve this, we:
        // * break HDR strokes into substrokes on load
        // * erase all substrokes of an HDR stroke when any substroke is erased
        // * assemble HDR substrokes back into single strokes on save
        // this field is a map from HDR substrokes to the full list of substrokes in that HDR
        public Dictionary<InkStroke, List<InkStroke>> splitStrokes;
        public Dictionary<ColorAlias, ColorRGB> theme;

        public DrawingContext() {
            strokes = new List<InkStroke>();
            splitStrokes = new Dictionary<InkStroke, List<InkStroke>>(new Draw.InkStrokeComparer());
        }
    }

    public static class Draw {
        // pressure * pen size * girth ratio = girth
        private const float girthRatio = 1.55f;
        // pressures below this pressure are rendered at this pressure
        private const float minPressure = 0.16f;
        // pressures above this pressure result in strange rending behavior
        private const float maxPressure = 1.0f;

        private static InkStrokeBuilder inkStrokeBuilder { get; } = new InkStrokeBuilder();

        private static Dictionary<ColorAlias, ColorRGB> defaultTheme = new Dictionary<ColorAlias, ColorRGB> {
            { ColorAlias.Black, new ColorRGB{r = 0xFF, g = 0xFF, b = 0xFF} }, // todo: reverse for light mode
            { ColorAlias.Red, new ColorRGB{r = 0xE8, g = 0x11, b = 0x23} },
            { ColorAlias.Green, new ColorRGB{r = 0x16, g = 0xC6, b = 0x0C} },
            { ColorAlias.Yellow, new ColorRGB{r = 0xFC, g = 0xE1, b = 0x00} },
            { ColorAlias.Blue, new ColorRGB{r = 0x00, g = 0x78, b = 0xD7} },
            { ColorAlias.Magenta, new ColorRGB{r = 0xE3, g = 0x00, b = 0x8C} },
            { ColorAlias.Cyan, new ColorRGB{r = 0x00, g = 0xB7, b = 0xC3} },
            { ColorAlias.White, new ColorRGB{r = 0x00, g = 0x00, b = 0x00} },
        };

        public static Color GetUIColor(this Dictionary<ColorAlias, ColorRGB> theme, ColorAlias color, float alpha) {
            if (theme == null || theme.Count == 0) {
                return defaultTheme.GetUIColor(color, alpha);
            }
            var alphaByte = (byte)Math.Min(alpha * 256, 255);
            var colorRGB = theme[color];
            return Color.FromArgb(alphaByte, colorRGB.r, colorRGB.g, colorRGB.b);
        }

        public static ColorAlias GetColorAlias(this Dictionary<ColorAlias, ColorRGB> theme, Color color) {
            if (theme == null || theme.Count == 0) {
                return defaultTheme.GetColorAlias(color);
            }
            var inverted = theme.Invert();
            return inverted[new ColorRGB { r = color.R, b = color.B, g = color.G }];
        }

        public static Dictionary<V, K> Invert<K, V>(this Dictionary<K, V> dict) {
            return dict.Keys.ToDictionary(key => dict.First(kvp => kvp.Key.Equals(key)).Value);
        }

        public static DrawingContext GetContext(this Drawing drawing) {
            var result = new DrawingContext {
                theme = drawing.theme,
            };

            foreach (var stroke in drawing.strokes) {
                var inkStrokes = SplitStroke(stroke, drawing.theme.GetUIColor(stroke.color, 1f)).ToList();
                result.strokes.AddRange(inkStrokes);
                foreach (var inkStroke in inkStrokes) {
                    result.splitStrokes[inkStroke] = inkStrokes;
                }
            }
            return result;
        }

        private static IEnumerable<InkStroke> SplitStroke(Stroke stroke, Color color) {
            var minGirth = float.MaxValue;
            var maxGirth = float.MinValue;
            var substrokeStart = 0;
            for (var i = 0; i < stroke.pointsGirth.Count; i++) {
                var girth = stroke.pointsGirth[i];
                if (girth < minGirth) {
                    minGirth = girth;
                }
                if (girth > maxGirth) {
                    maxGirth = girth;
                }
                if (maxGirth / minGirth > maxPressure / minPressure) {
                    // Stroke is HDR; emit a substroke for points with indices ∈ [substrokeStart, i)
                    // Select the pen size which assigns the lowest girth point to the minimum supported pressure
                    var size = PressureAndGirthToSize(minPressure, minGirth);
                    yield return SubStroke(stroke, color, size, substrokeStart, i);

                    // Point i was the point that would have made the substroke HDR
                    // Point i-1 was the last stroke we could include without doing that
                    // Point i-1 is where this substroke meets the next substroke and should be included in both
                    // If there are two consecutive points that form an HDR substroke, we can't connect them, so we leave a gap by not including point i in the next stroke
                    if (i != substrokeStart + 1) {
                        i--;
                    }
                    substrokeStart = i;

                    minGirth = stroke.pointsGirth[i];
                    maxGirth = stroke.pointsGirth[i];
                } else if (i == stroke.pointsGirth.Count - 1) {
                    // Assemble any remaining points into a final stroke
                    var size = PressureAndGirthToSize(minPressure, minGirth);
                    yield return SubStroke(stroke, color, size, substrokeStart, i + 1);
                }
            }
        }

        private static InkStroke SubStroke(Stroke stroke, Color color, float penSize, int startIndexInclusive, int endIndexExclusive) {
            var attributes = new InkDrawingAttributes();
            // Select the pen size which assigns the lowest girth point to the minimum supported pressure
            attributes.Size = new Size(penSize, penSize);
            attributes.Color = color;
            inkStrokeBuilder.SetDefaultDrawingAttributes(attributes);

            var inkPoints = new List<InkPoint>();
            for (var i = startIndexInclusive; i < endIndexExclusive; i++) {
                // Select pressures according to the selected pen size
                var pressure = SizeAndGirthToPressure(penSize, stroke.pointsGirth[i]);
                inkPoints.Add(new InkPoint(new Point(stroke.pointsX[i], stroke.pointsY[i]), pressure, 0, 0, 0));
            }
            return inkStrokeBuilder.CreateStrokeFromInkPoints(inkPoints, Matrix3x2.Identity);
        }

        public static Drawing GetDrawing(this DrawingContext context) {
            return new Drawing {
                scale = 1,
                translationX = 0,
                translationY = 0,
                strokes = JoinStrokes(context).ToList(),
                theme = context.theme,
            };
        }

        private static IEnumerable<Stroke> JoinStrokes(DrawingContext context) {
            var comparer = new InkStrokeComparer();
            var strokesToEmit = context.strokes.ToList();
            while (strokesToEmit.Count != 0) {
                var strokeToEmit = strokesToEmit[0];
                var connectedStrokes = context.splitStrokes[strokeToEmit];
                yield return JoinStroke(connectedStrokes, context.theme.GetColorAlias(strokeToEmit.DrawingAttributes.Color));
                foreach (var connectedStroke in connectedStrokes) {
                    strokesToEmit.Remove(strokesToEmit.First(stroke => comparer.Equals(stroke, connectedStroke)));
                }
            }
        }

        private static Stroke JoinStroke(List<InkStroke> substrokes, ColorAlias color) {
            var stroke = new Stroke {
                pointsX = new List<float>(),
                pointsY = new List<float>(),
                pointsGirth = new List<float>(),
                color = color,
                alpha = 0xFF,
            };
            InkPoint previousPoint = null;
            InkStroke previousInkStroke = null;
            foreach (var inkStroke in substrokes) {
                foreach (var point in inkStroke.GetInkPoints()) {
                    if (!InkPointsEqual(point, inkStroke, previousPoint, previousInkStroke)) {
                        stroke.pointsX.Add((float)point.Position.X);
                        stroke.pointsY.Add((float)point.Position.Y);
                        stroke.pointsGirth.Add(SizeAndPressureToGirth((float)inkStroke.DrawingAttributes.Size.Width, point.Pressure));
                        previousPoint = point;
                        previousInkStroke = inkStroke;
                    }
                }
            }
            return stroke;
        }

        private static float SizeAndPressureToGirth(double size, float pressure) {
            return SizeAndPressureToGirth((float)size, pressure);
        }

        private static float SizeAndPressureToGirth(float size, float pressure) {
            return size * pressure * girthRatio;
        }

        private static float SizeAndGirthToPressure(float size, float girth) {
            return Math.Clamp(girth / (size * girthRatio), minPressure, maxPressure);
        }

        private static float PressureAndGirthToSize(float pressure, float girth) {
            return girth / (pressure * girthRatio);
        }

        private static bool InkPointsEqual(InkPoint a, InkStroke aStroke, InkPoint b, InkStroke bStroke) {
            if(a == b) {
                return true;
            }
            if(a == null || b == null) {
                return false;
            }
            var girthDifference = SizeAndPressureToGirth(aStroke.DrawingAttributes.Size.Width, a.Pressure) - SizeAndPressureToGirth(bStroke.DrawingAttributes.Size.Width, b.Pressure);
            return
                a.Position.X == b.Position.X &&
                a.Position.Y == b.Position.Y &&
                Math.Abs(girthDifference) < 0.1f;
        }

        public class InkStrokeComparer : IEqualityComparer<InkStroke> {
            public bool Equals(InkStroke x, InkStroke y) {
                var xInkPoints = x.GetInkPoints();
                var yInkPoints = y.GetInkPoints();
                if (xInkPoints.Count != yInkPoints.Count) {
                    return false;
                }
                for (var i = 0; i < xInkPoints.Count; i++) {
                    if (!InkPointsEqual(xInkPoints[i], x, yInkPoints[i], y)) {
                        return false;
                    }
                }
                return true;
            }

            public int GetHashCode(InkStroke stroke) {
                var hashCode = 420;
                foreach(var inkPoint in stroke.GetInkPoints()) {
                    hashCode *= 69;
                    hashCode ^= inkPoint.Position.X.GetHashCode();
                    hashCode *= 69;
                    hashCode ^= inkPoint.Position.Y.GetHashCode();
                    // note: size not included because equality of sizes is not exact
                }
                return hashCode;
            }
        }
    }
}
