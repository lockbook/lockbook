# Themes
Currently our windows and linux clients are themable. The clients themselves will report where they are reading their themes from in **settings -> appearance**. 

You can add your own theme simply by placing it in the themes directory, the name of the json file will correspond to the theme name. Creating a theme involves selecting 18 colors that our app can draw from based on context.

This is our default theme:
```json
{
  "dim": {
    "black": "#101010",
    "grey": "#1D1D1D",
    "red": "#DF2040",
    "green": "#00B371",
    "yellow": "#E6AC00",
    "blue": "#207FDF",
    "magenta": "#7855AA",
    "cyan": "#00BBCC",
    "white": "#FFFFFF"
  },
  "light_prefs": {
    "primary": "blue",
    "secondary": "green",
    "tertiary": "magenta",
    "quaternary": "cyan"
  },
  "bright": {
    "black": "#101010",
    "grey": "#F6F6F6",
    "red": "#FF6680",
    "green": "#67E4B6",
    "yellow": "#FFDB70",
    "blue": "#66B2FF",
    "magenta": "#AC8CD9",
    "cyan": "#6EECF7",
    "white": "#FFFFFF"
  },
  "dark_prefs": {
    "primary": "blue",
    "secondary": "green",
    "tertiary": "magenta",
    "quaternary": "cyan"
  }
}
```


For reference this is the color palette visualized: 

| Color   | Dim                                                                 | Bright                                                               |
| ------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| black   | ![#101010](https://placehold.co/20x20/101010/101010.png) `#101010` | ![#101010](https://placehold.co/20x20/101010/101010.png) `#101010` |
| grey    | ![#1D1D1D](https://placehold.co/20x20/1D1D1D/1D1D1D.png) `#1D1D1D` | ![#F6F6F6](https://placehold.co/20x20/F6F6F6/F6F6F6.png) `#F6F6F6` |
| red     | ![#DF2040](https://placehold.co/20x20/DF2040/DF2040.png) `#DF2040` | ![#FF6680](https://placehold.co/20x20/FF6680/FF6680.png) `#FF6680` |
| green   | ![#00B371](https://placehold.co/20x20/00B371/00B371.png) `#00B371` | ![#67E4B6](https://placehold.co/20x20/67E4B6/67E4B6.png) `#67E4B6` |
| yellow  | ![#E6AC00](https://placehold.co/20x20/E6AC00/E6AC00.png) `#E6AC00` | ![#FFDB70](https://placehold.co/20x20/FFDB70/FFDB70.png) `#FFDB70` |
| blue    | ![#207FDF](https://placehold.co/20x20/207FDF/207FDF.png) `#207FDF` | ![#66B2FF](https://placehold.co/20x20/66B2FF/66B2FF.png) `#66B2FF` |
| magenta | ![#7855AA](https://placehold.co/20x20/7855AA/7855AA.png) `#7855AA` | ![#AC8CD9](https://placehold.co/20x20/AC8CD9/AC8CD9.png) `#AC8CD9` |
| cyan    | ![#00BBCC](https://placehold.co/20x20/00BBCC/00BBCC.png) `#00BBCC` | ![#6EECF7](https://placehold.co/20x20/6EECF7/6EECF7.png) `#6EECF7` |
| white   | ![#FFFFFF](https://placehold.co/20x20/FFFFFF/FFFFFF.png) `#FFFFFF` | ![#FFFFFF](https://placehold.co/20x20/FFFFFF/FFFFFF.png) `#FFFFFF` |

Some hints for designing good themes:
* In dark mode we draw from the dim palettes for backgrounds, and bright for foregrounds.
* In light mode we do the opposite (brights for backgrounds, dim for foreground).
* In dark mode we use black as the tab background, and white for the text.
* In light mode we use white for the tab background and white for the text.
* We always use grey for the sidebar color (but which grey depends on light/dark).
* We interpolate between grey and white or grey and black (depending on light/dark) to extract a handful more neutral colors.
* Some ideas are hardcoded to colors, for instance errors generally show up red, and warnings yellow. Other ideas we allow the user to express a preference. For example: the file tree, text selection, and syntax highlighting select colors based on what the user defines as their accent color preferences.
