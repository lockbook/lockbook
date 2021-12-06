using System;
using System.Collections.Generic;
using System.Linq;

namespace Core {
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

    public class InkStroke {
        public float size;
        public List<InkPoint> points;
        public ColorRGB color;
        public byte alpha;

        public InkStroke() {
            points = new List<InkPoint>();
        }
    }

    public class InkPoint {
        public float x;
        public float y;
        public float pressure;
    }

    public static class Draw {
        // (pressure + offset) * pen size * girth ratio = girth
        private const float girthRatio = 1.55f;
        private const float offset = 0.16f;
        // pressures below this pressure are rendered at this pressure
        private const float minPressure = 0.16f;
        // pressures above this pressure result in strange rendering behavior
        private const float maxPressure = 1.0f;

        public static Dictionary<ColorAlias, ColorRGB> defaultTheme = new() {
            { ColorAlias.Black, new ColorRGB{r = 0xFF, g = 0xFF, b = 0xFF} }, // todo: reverse for light mode
            { ColorAlias.Red, new ColorRGB{r = 0xE8, g = 0x11, b = 0x23} },
            { ColorAlias.Green, new ColorRGB{r = 0x16, g = 0xC6, b = 0x0C} },
            { ColorAlias.Yellow, new ColorRGB{r = 0xFC, g = 0xE1, b = 0x00} },
            { ColorAlias.Blue, new ColorRGB{r = 0x00, g = 0x78, b = 0xD7} },
            { ColorAlias.Magenta, new ColorRGB{r = 0xE3, g = 0x00, b = 0x8C} },
            { ColorAlias.Cyan, new ColorRGB{r = 0x00, g = 0xB7, b = 0xC3} },
            { ColorAlias.White, new ColorRGB{r = 0x00, g = 0x00, b = 0x00} },
        };

        public static DrawingContext DrawingToCoreContext(Drawing drawing) {
            var result = new DrawingContext {
                theme = drawing.theme,
            };

            foreach (var stroke in drawing.strokes) {
                var inkStrokes = SplitStroke(stroke).Select(strokeSplit => StrokeToCoreInkStroke(strokeSplit, drawing.theme)).ToList();
                result.strokes.AddRange(inkStrokes);
                foreach (var inkStroke in inkStrokes) {
                    result.splitStrokes[inkStroke] = inkStrokes;
                }
            }
            return result;
        }

        public static Drawing CoreContextToDrawing(DrawingContext coreContext) {
            return new Drawing {
                scale = 1,
                translationX = 0,
                translationY = 0,
                strokes = JoinStrokes(coreContext).ToList(),
                theme = coreContext.theme,
            };
        }

        private static InkStroke StrokeToCoreInkStroke(Stroke stroke, Dictionary<ColorAlias, ColorRGB> theme) {
            // Select the pen size which assigns the minimum supported pressure to the lowest girth point
            var size = PressureAndGirthToSize(minPressure, stroke.pointsGirth.Min());
            var points = new List<InkPoint>();
            for (var i = 0; i < stroke.pointsGirth.Count; i++) {
                points.Add(new InkPoint {
                    x = stroke.pointsX[i],
                    y = stroke.pointsY[i],
                    pressure = SizeAndGirthToPressure(size, stroke.pointsGirth[i]),
                });
            }
            return new InkStroke {
                size = size,
                points = points,
                color = theme.ColorAliasToRGB(stroke.color),
                alpha = (byte)Math.Min(stroke.alpha * 256, 255),
            };
        }

        private static Stroke CoreStrokeToStroke(InkStroke stroke, Dictionary<ColorAlias, ColorRGB> theme) {
            return new Stroke {
                pointsX = stroke.points.Select(point => point.x).ToList(),
                pointsY = stroke.points.Select(point => point.y).ToList(),
                pointsGirth = stroke.points.Select(point => SizeAndPressureToGirth(stroke.size, point.pressure)).ToList(),
                color = theme.RGBColorToAlias(stroke.color),
                alpha = ((float)stroke.alpha) / 255,
            };
        }

        public static ColorRGB ColorAliasToRGB(this Dictionary<ColorAlias, ColorRGB> theme, ColorAlias color) {
            if (theme == null || theme.Count == 0) {
                return defaultTheme[color];
            }
            return theme[color];
        }

        public static ColorAlias RGBColorToAlias(this Dictionary<ColorAlias, ColorRGB> theme, ColorRGB color) {
            if (color == null) {
                return ColorAlias.Black;
            }
            if (theme == null || theme.Count == 0) {
                return defaultTheme.Invert()[color];
            }
            return theme.Invert()[color];
        }

        public static Dictionary<V, K> Invert<K, V>(this Dictionary<K, V> dict) {
            return dict.Keys.ToDictionary(key => dict.First(kvp => kvp.Key.Equals(key)).Value);
        }

        private static IEnumerable<Stroke> SplitStroke(Stroke stroke) {
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
                if (maxGirth / minGirth > SizeAndPressureToGirth(1, maxPressure) / SizeAndPressureToGirth(1, minPressure)) {
                    // Stroke is HDR; emit a substroke for points with indices ∈ [substrokeStart, i)
                    yield return SubStroke(stroke, substrokeStart, i);

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
                    yield return SubStroke(stroke, substrokeStart, i + 1);
                }
            }
        }

        private static Stroke SubStroke(Stroke stroke, int startIndexInclusive, int endIndexExclusive) {
            return new Stroke {
                pointsX = stroke.pointsX.Skip(startIndexInclusive).Take(endIndexExclusive).ToList(),
                pointsY = stroke.pointsY.Skip(startIndexInclusive).Take(endIndexExclusive).ToList(),
                pointsGirth = stroke.pointsGirth.Skip(startIndexInclusive).Take(endIndexExclusive).ToList(),
                color = stroke.color,
                alpha = stroke.alpha,
            };
        }

        private static IEnumerable<Stroke> JoinStrokes(DrawingContext context) {
            var comparer = new InkStrokeComparer();
            var strokesToEmit = context.strokes.ToList();
            while (strokesToEmit.Count != 0) {
                var strokeToEmit = strokesToEmit[0];
                var connectedStrokes = context.splitStrokes[strokeToEmit];
                yield return JoinStroke(connectedStrokes.Select(stroke => CoreStrokeToStroke(stroke, context.theme)), context.theme.RGBColorToAlias(strokeToEmit.color));
                foreach (var connectedStroke in connectedStrokes) {
                    strokesToEmit.Remove(strokesToEmit.First(stroke => comparer.Equals(stroke, connectedStroke)));
                }
            }
        }

        private static Stroke JoinStroke(IEnumerable<Stroke> substrokes, ColorAlias color) {
            var stroke = new Stroke {
                pointsX = new List<float>(),
                pointsY = new List<float>(),
                pointsGirth = new List<float>(),
                color = color,
                alpha = 0xFF,
            };
            var previousX = -420.69f;
            var previousY = -420.69f;
            var previousGirth = -420.69f;
            foreach (var substroke in substrokes) {
                for (var i = 0; i < substroke.pointsX.Count; i++) {
                    if (!PointsEqual(substroke.pointsX[i], substroke.pointsY[i], substroke.pointsGirth[i], previousX, previousY, previousGirth)) {
                        stroke.pointsX.Add(substroke.pointsX[i]);
                        stroke.pointsY.Add(substroke.pointsY[i]);
                        stroke.pointsGirth.Add(substroke.pointsGirth[i]);
                        previousX = substroke.pointsX[i];
                        previousY = substroke.pointsY[i];
                        previousGirth = substroke.pointsGirth[i];
                    }
                }
            }
            return stroke;
        }

        public static float SizeAndPressureToGirth(float size, float pressure) {
            return size * (pressure + offset) * girthRatio;
        }

        public static float SizeAndPressureToGirth(double size, float pressure) {
            return SizeAndPressureToGirth((float)size, pressure);
        }

        private static float SizeAndGirthToPressure(float size, float girth) {
            return Clamp(girth / (size * girthRatio) - offset, minPressure, maxPressure);
        }

        private static T Clamp<T>(T v, T min, T max) where T : IComparable<T> {
            if (v.CompareTo(min) < 0) return min;
            else if (v.CompareTo(max) > 0) return max;
            else return v;
        }

        private static float PressureAndGirthToSize(float pressure, float girth) {
            return girth / ((pressure + offset) * girthRatio);
        }

        public static bool PointsEqual(float aX, float aY, float aGirth, float bX, float bY, float bGirth) {
            return aX == bX && aY == bY && Math.Abs(aGirth - bGirth) < .1f;
        }

        public static bool PointsEqual(double aX, double aY, float aGirth, double bX, double bY, float bGirth) {
            return PointsEqual(aX, aY, aGirth, bX, bY, bGirth);
        }

        private static bool CoreInkPointsEqual(InkPoint a, InkStroke aStroke, InkPoint b, InkStroke bStroke) {
            if (a == b) {
                return true;
            }
            if (a == null || b == null) {
                return false;
            }
            var aGirth = SizeAndPressureToGirth(aStroke.size, a.pressure);
            var bGirth = SizeAndPressureToGirth(bStroke.size, b.pressure);
            return PointsEqual(a.x, a.y, aGirth, b.x, b.y, bGirth);
        }

        public class InkStrokeComparer : IEqualityComparer<InkStroke> {
            public bool Equals(InkStroke x, InkStroke y) {
                if (x.points.Count != y.points.Count) {
                    return false;
                }
                for (var i = 0; i < x.points.Count; i++) {
                    if (!CoreInkPointsEqual(x.points[i], x, y.points[i], y)) {
                        return false;
                    }
                }
                return true;
            }

            public int GetHashCode(InkStroke stroke) {
                var hashCode = 420;
                foreach (var inkPoint in stroke.points) {
                    hashCode *= 69;
                    hashCode ^= inkPoint.x.GetHashCode();
                    hashCode *= 69;
                    hashCode ^= inkPoint.y.GetHashCode();
                    // note: size not included because equality of sizes is not exact
                }
                return hashCode;
            }
        }
    }
}
