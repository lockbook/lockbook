using Core;
using lib;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using Windows.Foundation;
using Windows.UI.Input.Inking;

namespace devtest {
    [TestClass]
    public class DrawTest {
        private static InkStrokeBuilder inkStrokeBuilder1 { get; } = new InkStrokeBuilder();
        private static InkStrokeBuilder inkStrokeBuilder2 { get; } = new InkStrokeBuilder();

        private bool StrokeEqual(InkStroke expected, InkStroke actual) {
            if (Math.Abs(expected.DrawingAttributes.Size.Width - actual.DrawingAttributes.Size.Width) > 0.01f) {
                return false;
            }
            if (Math.Abs(expected.DrawingAttributes.Size.Height - actual.DrawingAttributes.Size.Height) > 0.01f) {
                return false;
            }
            var expectedInkPoints = expected.GetInkPoints();
            var actualInkPoints = actual.GetInkPoints();
            if (expectedInkPoints.Count != actualInkPoints.Count) {
                return false;
            }
            for (var j = 0; j < expectedInkPoints.Count; j++) {
                var expectedInkPoint = expectedInkPoints[j];
                var actualInkPoint = actualInkPoints[j];
                if (expectedInkPoint.Position.X != actualInkPoint.Position.X) {
                    return false;
                }
                if (expectedInkPoint.Position.Y != actualInkPoint.Position.Y) {
                    return false;
                }
                if (Math.Abs(expectedInkPoint.Pressure - actualInkPoint.Pressure) > 0.01f) {
                    return false;
                }
            }
            return true;
        }

        private void AssertStrokeEqual(InkStroke expected, InkStroke actual) {
            Assert.IsTrue(StrokeEqual(expected, actual));
        }

        private void AssertStrokesEqual(IReadOnlyList<InkStroke> expected, IReadOnlyList<InkStroke> actual) {
            Assert.AreEqual(expected.Count(), actual.Count());
            for (var i = 0; i < expected.Count(); i++) {
                AssertStrokeEqual(expected[i], actual[i]);
            }
        }

        private void AssertSplitStrokesEqual(Dictionary<InkStroke, List<InkStroke>> expected, Dictionary<InkStroke, List<InkStroke>> actual) {
            Assert.AreEqual(expected.Keys.Where(expectedKey => actual.Keys.Count(actualKey => StrokeEqual(expectedKey, actualKey)) == 0).Count(), 0);
            Assert.AreEqual(actual.Keys.Where(actualKey => expected.Keys.Count(expectedKey => StrokeEqual(expectedKey, actualKey)) == 0).Count(), 0);
            foreach (var key in expected.Keys) {
                AssertStrokesEqual(expected[key], actual[actual.Keys.First(actualKey => StrokeEqual(key, actualKey))]);
            }
        }

        private void AssertContextEqual(DrawingContext expected, DrawingContext actual) {
            AssertStrokesEqual(expected.strokes, actual.strokes);
            AssertSplitStrokesEqual(expected.splitStrokes, actual.splitStrokes);
            // note: ignores theme
        }

        private void AssertDrawingEqual(Drawing expected, Drawing actual) {
            Assert.AreEqual(expected.strokes.Count, actual.strokes.Count);
            for (var i = 0; i < expected.strokes.Count; i++) {
                var expectedStroke = expected.strokes[i];
                var actualStroke = actual.strokes[i];
                Assert.AreEqual(expectedStroke.pointsX.Count, actualStroke.pointsX.Count);
                Assert.AreEqual(expectedStroke.pointsY.Count, actualStroke.pointsY.Count);
                Assert.AreEqual(expectedStroke.pointsGirth.Count, actualStroke.pointsGirth.Count);
                for (var j = 0; j < expectedStroke.pointsX.Count; j++) {
                    Assert.AreEqual(expectedStroke.pointsX[j], actualStroke.pointsX[j]);
                    Assert.AreEqual(expectedStroke.pointsY[j], actualStroke.pointsY[j]);
                    Assert.IsTrue(Math.Abs(expectedStroke.pointsGirth[j] - actualStroke.pointsGirth[j]) < 0.1f);
                }
                // note: ignores color, alpha
            }
        }

        [TestMethod]
        public void GetContext() {
            var drawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2},
                        pointsY = new List<float> {0, 0, 0},
                        pointsGirth = new List<float> {1.55f, 1.55f, 1.55f},
                    }
                }
            };
            inkStrokeBuilder1.SetDefaultDrawingAttributes(new InkDrawingAttributes { Size = new Size(6.25f, 6.25f) });
            var expectedContext = new DrawingContext {
                strokes = new List<InkStroke> {
                    inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                        new InkPoint(new Point(0, 0), .16f),
                        new InkPoint(new Point(1, 0), .16f),
                        new InkPoint(new Point(2, 0), .16f),
                    }, Matrix3x2.Identity),
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>> {
                    {
                        inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                            new InkPoint(new Point(0, 0), .16f),
                            new InkPoint(new Point(1, 0), .16f),
                            new InkPoint(new Point(2, 0), .16f),
                        }, Matrix3x2.Identity),
                        new List<InkStroke> {
                            inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(0, 0), .16f),
                                new InkPoint(new Point(1, 0), .16f),
                                new InkPoint(new Point(2, 0), .16f),
                            }, Matrix3x2.Identity)
                        }
                    }
                }
            };

            var actualContext = drawing.GetContext();

            AssertContextEqual(expectedContext, actualContext);
        }

        [TestMethod]
        public void GetContextSplit() {
            var drawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2},
                        pointsY = new List<float> {0, 0, 0},
                        pointsGirth = new List<float> {1.55f, 1.55f * 4, 1.55f * 8},
                    }
                }
            };
            inkStrokeBuilder1.SetDefaultDrawingAttributes(new InkDrawingAttributes { Size = new Size(6.25f, 6.25f) });
            inkStrokeBuilder2.SetDefaultDrawingAttributes(new InkDrawingAttributes { Size = new Size(6.25f * 4, 6.25f * 4) });
            var expectedContext = new DrawingContext {
                strokes = new List<InkStroke> {
                    inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                        new InkPoint(new Point(0, 0), .16f),
                        new InkPoint(new Point(1, 0), .16f * 4),
                    }, Matrix3x2.Identity),
                    inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                        new InkPoint(new Point(1, 0), .16f),
                        new InkPoint(new Point(2, 0), .16f * 2),
                    }, Matrix3x2.Identity),
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>> {
                    {
                        inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                            new InkPoint(new Point(0, 0), .16f),
                            new InkPoint(new Point(1, 0), .16f * 4),
                        }, Matrix3x2.Identity),
                        new List<InkStroke> {
                            inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(0, 0), .16f),
                                new InkPoint(new Point(1, 0), .16f * 4),
                            }, Matrix3x2.Identity),
                            inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(1, 0), .16f),
                                new InkPoint(new Point(2, 0), .16f * 2),
                            }, Matrix3x2.Identity),
                        }
                    },
                    {
                        inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                            new InkPoint(new Point(1, 0), .16f),
                            new InkPoint(new Point(2, 0), .16f * 2),
                        }, Matrix3x2.Identity),
                        new List<InkStroke> {
                            inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(0, 0), .16f),
                                new InkPoint(new Point(1, 0), .16f * 4),
                            }, Matrix3x2.Identity),
                            inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(1, 0), .16f),
                                new InkPoint(new Point(2, 0), .16f * 2),
                            }, Matrix3x2.Identity),
                        }
                    },
                }
            };

            var actualContext = drawing.GetContext();

            AssertContextEqual(expectedContext, actualContext);
        }

        [TestMethod]
        public void GetDrawing() {
            inkStrokeBuilder1.SetDefaultDrawingAttributes(new InkDrawingAttributes { Size = new Size(6.25f, 6.25f) });
            var context = new DrawingContext {
                strokes = new List<InkStroke> {
                    inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                        new InkPoint(new Point(0, 0), .16f),
                        new InkPoint(new Point(1, 0), .16f),
                        new InkPoint(new Point(2, 0), .16f),
                    }, Matrix3x2.Identity),
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>>(new Draw.InkStrokeComparer()) {
                    {
                        inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                            new InkPoint(new Point(0, 0), .16f),
                            new InkPoint(new Point(1, 0), .16f),
                            new InkPoint(new Point(2, 0), .16f),
                        }, Matrix3x2.Identity),
                        new List<InkStroke> {
                            inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(0, 0), .16f),
                                new InkPoint(new Point(1, 0), .16f),
                                new InkPoint(new Point(2, 0), .16f),
                            }, Matrix3x2.Identity)
                        }
                    }
                }
            };
            var expectedDrawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2},
                        pointsY = new List<float> {0, 0, 0},
                        pointsGirth = new List<float> {1.55f, 1.55f, 1.55f},
                    }
                }
            };

            var actualDrawing = context.GetDrawing();

            AssertDrawingEqual(expectedDrawing, actualDrawing);
        }

        [TestMethod]
        public void GetDrawingSplit() {
            inkStrokeBuilder1.SetDefaultDrawingAttributes(new InkDrawingAttributes { Size = new Size(6.25f, 6.25f) });
            inkStrokeBuilder2.SetDefaultDrawingAttributes(new InkDrawingAttributes { Size = new Size(6.25f * 4, 6.25f * 4) });
            var context = new DrawingContext {
                strokes = new List<InkStroke> {
                    inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                        new InkPoint(new Point(0, 0), .16f),
                        new InkPoint(new Point(1, 0), .16f * 4),
                    }, Matrix3x2.Identity),
                    inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                        new InkPoint(new Point(1, 0), .16f),
                        new InkPoint(new Point(2, 0), .16f * 2),
                    }, Matrix3x2.Identity),
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>>(new Draw.InkStrokeComparer()) {
                    {
                        inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                            new InkPoint(new Point(0, 0), .16f),
                            new InkPoint(new Point(1, 0), .16f * 4),
                        }, Matrix3x2.Identity),
                        new List<InkStroke> {
                            inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(0, 0), .16f),
                                new InkPoint(new Point(1, 0), .16f * 4),
                            }, Matrix3x2.Identity),
                            inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(1, 0), .16f),
                                new InkPoint(new Point(2, 0), .16f * 2),
                            }, Matrix3x2.Identity),
                        }
                    },
                    {
                        inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                            new InkPoint(new Point(1, 0), .16f),
                            new InkPoint(new Point(2, 0), .16f * 2),
                        }, Matrix3x2.Identity),
                        new List<InkStroke> {
                            inkStrokeBuilder1.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(0, 0), .16f),
                                new InkPoint(new Point(1, 0), .16f * 4),
                            }, Matrix3x2.Identity),
                            inkStrokeBuilder2.CreateStrokeFromInkPoints(new List<InkPoint>{
                                new InkPoint(new Point(1, 0), .16f),
                                new InkPoint(new Point(2, 0), .16f * 2),
                            }, Matrix3x2.Identity),
                        }
                    },
                }
            };
            var expectedDrawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2},
                        pointsY = new List<float> {0, 0, 0},
                        pointsGirth = new List<float> {1.55f, 1.55f * 4, 1.55f * 8},
                    }
                }
            };

            var actualDrawing = context.GetDrawing();

            AssertDrawingEqual(expectedDrawing, actualDrawing);
        }
    }
}
