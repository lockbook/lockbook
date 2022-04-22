using Core;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;
using System.Collections.Generic;
using System.Linq;

namespace test {
    [TestClass]
    public class DrawTest {
        private bool StrokeEqual(InkStroke expected, InkStroke actual) {
            return new Draw.InkStrokeComparer().Equals(expected, actual);
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
                        pointsGirth = new List<float> {3.1f, 3.1f, 3.1f},
                    }
                }
            };
            var expectedContext = new DrawingContext {
                strokes = new List<InkStroke> {
                    new InkStroke {
                        size = 6.25f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 0, y = 0, pressure = .16f },
                            new InkPoint { x = 1, y = 0, pressure = .16f },
                            new InkPoint { x = 2, y = 0, pressure = .16f },
                        },
                    },
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>> {
                    {
                        new InkStroke {
                            size = 6.25f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 0, y = 0, pressure = .16f },
                                new InkPoint { x = 1, y = 0, pressure = .16f },
                                new InkPoint { x = 2, y = 0, pressure = .16f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                },
                            },
                        }
                    }
                }
            };

            var actualContext = Draw.DrawingToCoreContext(drawing);

            AssertContextEqual(expectedContext, actualContext);
        }

        [TestMethod]
        public void GetContextSplit() {
            var drawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2, 3},
                        pointsY = new List<float> {0, 0, 0, 0},
                        pointsGirth = new List<float> {3.1f, 3.1f * 3, 3.1f * 9, 3.1f * 27},
                    }
                }
            };
            var expectedContext = new DrawingContext {
                strokes = new List<InkStroke> {
                    new InkStroke {
                        size = 6.25f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 0, y = 0, pressure = .16f },
                            new InkPoint { x = 1, y = 0, pressure = .80f },
                        },
                    },
                    new InkStroke {
                        size = 18.75f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 1, y = 0, pressure = .16f },
                            new InkPoint { x = 2, y = 0, pressure = .80f },
                        },
                    },
                    new InkStroke {
                        size = 56.25f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 2, y = 0, pressure = .16f },
                            new InkPoint { x = 3, y = 0, pressure = .80f },
                        },
                    },
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>> {
                    {
                        new InkStroke {
                            size = 6.25f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 0, y = 0, pressure = .16f },
                                new InkPoint { x = 1, y = 0, pressure = .80f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 18.75f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 56.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                    new InkPoint { x = 3, y = 0, pressure = .80f },
                                },
                            },
                        }
                    },
                    {
                        new InkStroke {
                            size = 18.75f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 1, y = 0, pressure = .16f },
                                new InkPoint { x = 2, y = 0, pressure = .80f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 18.75f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 56.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                    new InkPoint { x = 3, y = 0, pressure = .80f },
                                },
                            },
                        }
                    },
                    {
                        new InkStroke {
                            size = 56.25f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 2, y = 0, pressure = .16f },
                                new InkPoint { x = 3, y = 0, pressure = .80f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 18.75f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 56.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                    new InkPoint { x = 3, y = 0, pressure = .80f },
                                },
                            },
                        }
                    },
                }
            };

            var actualContext = Draw.DrawingToCoreContext(drawing);

            AssertContextEqual(expectedContext, actualContext);
        }

        [TestMethod]
        public void GetDrawing() {
            var context = new DrawingContext {
                strokes = new List<InkStroke> {
                    new InkStroke {
                        size = 6.25f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 0, y = 0, pressure = .16f },
                            new InkPoint { x = 1, y = 0, pressure = .16f },
                            new InkPoint { x = 2, y = 0, pressure = .16f },
                        },
                    },
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>>(new Draw.InkStrokeComparer()) {
                    {
                        new InkStroke {
                            size = 6.25f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 0, y = 0, pressure = .16f },
                                new InkPoint { x = 1, y = 0, pressure = .16f },
                                new InkPoint { x = 2, y = 0, pressure = .16f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                },
                            },
                        }
                    }
                }
            };
            var expectedDrawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2},
                        pointsY = new List<float> {0, 0, 0},
                        pointsGirth = new List<float> {3.1f, 3.1f, 3.1f},
                    }
                }
            };

            var actualDrawing = Draw.CoreContextToDrawing(context);

            AssertDrawingEqual(expectedDrawing, actualDrawing);
        }

        [TestMethod]
        public void GetDrawingSplit() {
            var context = new DrawingContext {
                strokes = new List<InkStroke> {
                    new InkStroke {
                        size = 6.25f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 0, y = 0, pressure = .16f },
                            new InkPoint { x = 1, y = 0, pressure = .80f },
                        },
                    },
                    new InkStroke {
                        size = 18.75f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 1, y = 0, pressure = .16f },
                            new InkPoint { x = 2, y = 0, pressure = .80f },
                        },
                    },
                    new InkStroke {
                        size = 56.25f,
                        points = new List<InkPoint> {
                            new InkPoint { x = 2, y = 0, pressure = .16f },
                            new InkPoint { x = 3, y = 0, pressure = .80f },
                        },
                    },
                },
                splitStrokes = new Dictionary<InkStroke, List<InkStroke>>(new Draw.InkStrokeComparer()) {
                    {
                        new InkStroke {
                            size = 6.25f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 0, y = 0, pressure = .16f },
                                new InkPoint { x = 1, y = 0, pressure = .80f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 18.75f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 56.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                    new InkPoint { x = 3, y = 0, pressure = .80f },
                                },
                            },
                        }
                    },
                    {
                        new InkStroke {
                            size = 18.75f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 1, y = 0, pressure = .16f },
                                new InkPoint { x = 2, y = 0, pressure = .80f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 18.75f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 56.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                    new InkPoint { x = 3, y = 0, pressure = .80f },
                                },
                            },
                        }
                    },
                    {
                        new InkStroke {
                            size = 56.25f,
                            points = new List<InkPoint> {
                                new InkPoint { x = 2, y = 0, pressure = .16f },
                                new InkPoint { x = 3, y = 0, pressure = .80f },
                            },
                        },
                        new List<InkStroke> {
                            new InkStroke {
                                size = 6.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 0, y = 0, pressure = .16f },
                                    new InkPoint { x = 1, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 18.75f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 1, y = 0, pressure = .16f },
                                    new InkPoint { x = 2, y = 0, pressure = .80f },
                                },
                            },
                            new InkStroke {
                                size = 56.25f,
                                points = new List<InkPoint> {
                                    new InkPoint { x = 2, y = 0, pressure = .16f },
                                    new InkPoint { x = 3, y = 0, pressure = .80f },
                                },
                            },
                        }
                    },
                }
            };
            var expectedDrawing = new Drawing {
                strokes = new List<Stroke> {
                    new Stroke {
                        pointsX = new List<float> {0, 1, 2, 3},
                        pointsY = new List<float> {0, 0, 0, 0},
                        pointsGirth = new List<float> {3.1f, 3.1f * 3, 3.1f * 9, 3.1f * 27},
                    }
                }
            };

            var actualDrawing = Draw.CoreContextToDrawing(context);

            AssertDrawingEqual(expectedDrawing, actualDrawing);
        }
    }
}
