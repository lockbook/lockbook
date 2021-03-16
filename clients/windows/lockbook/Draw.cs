using Core;
using System;
using System.Collections.Generic;
using Windows.UI;
using System.Linq;

namespace lockbook {
    public static class Draw {
        private static Dictionary<ColorAlias, ColorRGB> defaultTheme = new Dictionary<ColorAlias, ColorRGB> {
            { ColorAlias.Black, new ColorRGB{r = 0x00, g = 0x00, b = 0x00} },
            { ColorAlias.Red, new ColorRGB{r = 0xE8, g = 0x11, b = 0x23} },
            { ColorAlias.Green, new ColorRGB{r = 0x16, g = 0xC6, b = 0x0C} },
            { ColorAlias.Yellow, new ColorRGB{r = 0xFC, g = 0xE1, b = 0x00} },
            { ColorAlias.Blue, new ColorRGB{r = 0x00, g = 0x78, b = 0xD7} },
            { ColorAlias.Magenta, new ColorRGB{r = 0xE3, g = 0x00, b = 0x8C} },
            { ColorAlias.Cyan, new ColorRGB{r = 0x00, g = 0xB7, b = 0xC3} },
            { ColorAlias.White, new ColorRGB{r = 0xFF, g = 0xFF, b = 0xFF} },
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
            return inverted[new ColorRGB{r = color.R, b = color.B, g = color.G}];
        }

        public static Dictionary<V, K> Invert<K, V>(this Dictionary<K, V> dict) {
            return dict.Keys.ToDictionary(key => dict.First(kvp => kvp.Key.Equals(key)).Value);
        }
    }
}
