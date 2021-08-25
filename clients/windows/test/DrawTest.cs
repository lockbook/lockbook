using Core;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;
using System.Collections.Generic;
using System.Linq;
using Windows.UI.Input.Inking;

namespace lockbook {
    [TestClass]
    class DrawTest {
        private void AssertStrokeEqual(InkStroke expected, InkStroke actual) {
            var expectedInkPoints = expected.GetInkPoints();
            var actualInkPoints = actual.GetInkPoints();
            Assert.AreEqual(expectedInkPoints.Count, actualInkPoints.Count);
            for (var j = 0; j < expectedInkPoints.Count; j++) {
                var expectedInkPoint = expectedInkPoints[j];
                var actualInkPoint = actualInkPoints[j];
                Assert.AreEqual(expectedInkPoint.Position.X, actualInkPoint.Position.X);
                Assert.AreEqual(expectedInkPoint.Position.Y, actualInkPoint.Position.Y);
                Assert.IsTrue(Math.Abs(expectedInkPoint.Pressure - actualInkPoint.Pressure) < 0.1f);
            }
        }

        private void AssertStrokesEqual(IReadOnlyList<InkStroke> expected, IReadOnlyList<InkStroke> actual) {
            Assert.AreEqual(expected.Count(), actual.Count());
            for (var i = 0; i < expected.Count(); i++) {
                AssertStrokeEqual(expected[i], actual[i]);
            }
        }

        private void AssertSplitStrokesEqual(Dictionary<InkStroke, List<InkStroke>> expected, Dictionary<InkStroke, List<InkStroke>> actual) {
            Assert.AreEqual(expected.Keys.Where(key => !actual.Keys.Contains(key)).Count(), 0);
            Assert.AreEqual(actual.Keys.Where(key => !expected.Keys.Contains(key)).Count(), 0);
            foreach (var key in expected.Keys) {
                AssertStrokesEqual(expected[key], actual[key]);
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
                // todo
            };
            var expectedContext = new DrawingContext {
                // todo
            };

            var actualContext = drawing.GetContext();

            AssertContextEqual(expectedContext, actualContext);
        }

        [TestMethod]
        public void GetDrawing() {
            var context = new DrawingContext {
                // todo
            };
            var expectedDrawing = new Drawing {
                // todo
            };

            var actualDrawing = context.GetDrawing();

            AssertDrawingEqual(expectedDrawing, actualDrawing);
        }

        [TestMethod]
        public void GetContextGetDrawingSameDrawing() {
            var drawings = new List<Drawing> {
                // todo
            };
            foreach (var drawing in drawings) {
                AssertDrawingEqual(drawing, drawing.GetContext().GetDrawing());
            }
        }

        [TestMethod]
        public void GetDrawingGetContextSameContext() {
            var contexts = new List<DrawingContext> {
                // todo
            };
            foreach (var context in contexts) {
                AssertContextEqual(context, context.GetDrawing().GetContext());
            }
        }
    }
}
