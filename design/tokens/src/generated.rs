// Generated from DESIGN.md by `cargo xtask design`. Do not edit.

/// The Radix scales a host or widget color can come from, in Radix's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scale {
    Tomato,
    Red,
    Ruby,
    Crimson,
    Pink,
    Plum,
    Purple,
    Violet,
    Iris,
    Indigo,
    Blue,
    Cyan,
    Teal,
    Jade,
    Green,
    Grass,
    Brown,
    Bronze,
    Gold,
    Sky,
    Mint,
    Lime,
    Yellow,
    Amber,
    Orange,
}

impl Scale {
    pub const ALL: [Scale; 25] = [
        Scale::Tomato,
        Scale::Red,
        Scale::Ruby,
        Scale::Crimson,
        Scale::Pink,
        Scale::Plum,
        Scale::Purple,
        Scale::Violet,
        Scale::Iris,
        Scale::Indigo,
        Scale::Blue,
        Scale::Cyan,
        Scale::Teal,
        Scale::Jade,
        Scale::Green,
        Scale::Grass,
        Scale::Brown,
        Scale::Bronze,
        Scale::Gold,
        Scale::Sky,
        Scale::Mint,
        Scale::Lime,
        Scale::Yellow,
        Scale::Amber,
        Scale::Orange,
    ];

    /// Radix's name for the scale.
    pub const fn name(self) -> &'static str {
        match self {
            Scale::Tomato => "tomato",
            Scale::Red => "red",
            Scale::Ruby => "ruby",
            Scale::Crimson => "crimson",
            Scale::Pink => "pink",
            Scale::Plum => "plum",
            Scale::Purple => "purple",
            Scale::Violet => "violet",
            Scale::Iris => "iris",
            Scale::Indigo => "indigo",
            Scale::Blue => "blue",
            Scale::Cyan => "cyan",
            Scale::Teal => "teal",
            Scale::Jade => "jade",
            Scale::Green => "green",
            Scale::Grass => "grass",
            Scale::Brown => "brown",
            Scale::Bronze => "bronze",
            Scale::Gold => "gold",
            Scale::Sky => "sky",
            Scale::Mint => "mint",
            Scale::Lime => "lime",
            Scale::Yellow => "yellow",
            Scale::Amber => "amber",
            Scale::Orange => "orange",
        }
    }
}

/// The Radix gray families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GrayFamily {
    Gray,
    Mauve,
    Slate,
    Sage,
    Olive,
    Sand,
}

impl GrayFamily {
    pub const ALL: [GrayFamily; 6] = [
        GrayFamily::Gray,
        GrayFamily::Mauve,
        GrayFamily::Slate,
        GrayFamily::Sage,
        GrayFamily::Olive,
        GrayFamily::Sand,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            GrayFamily::Gray => "gray",
            GrayFamily::Mauve => "mauve",
            GrayFamily::Slate => "slate",
            GrayFamily::Sage => "sage",
            GrayFamily::Olive => "olive",
            GrayFamily::Sand => "sand",
        }
    }
}

/// OKLCH hue in degrees and chroma of light step 9, per `Scale::ALL`.
pub(crate) const HUE: [f64; 25] = [33.3390, 23.0261, 13.1504, 1.2762, 346.0031, 322.1066, 305.8601, 288.0332, 278.2846, 267.0054, 251.7799, 221.7438, 181.9625, 170.7301, 157.6795, 147.3918, 60.9842, 44.1621, 77.6986, 217.8045, 177.9783, 126.0886, 100.9361, 84.1300, 45.0151];
pub(crate) const CHROMA: [f64; 25] = [0.1936, 0.1933, 0.1949, 0.2130, 0.2076, 0.1877, 0.1829, 0.1790, 0.1841, 0.1910, 0.1930, 0.1217, 0.1136, 0.1150, 0.1329, 0.1468, 0.0785, 0.0460, 0.0492, 0.1027, 0.0999, 0.1747, 0.1838, 0.1572, 0.1909];

/// picks: the hue rule and what counts as neutral.
pub(crate) const HUE_DISTANCE: f64 = 30.0;
pub(crate) const NEUTRAL_BELOW_CHROMA: f64 = 0.08;
/// scales.status: ok, warning, error.
pub(crate) const STATUS: [Scale; 3] = [Scale::Green, Scale::Amber, Scale::Red];
/// picks.default.
pub(crate) const DEFAULT_GRAY: GrayFamily = GrayFamily::Gray;
pub(crate) const DEFAULT_HOST: Option<Scale> = None;
pub(crate) const DEFAULT_WIDGET: Scale = Scale::Blue;
pub(crate) const DEFAULT_RADIUS: crate::Radius = crate::Radius::Medium;
pub(crate) const DEFAULT_THEME: crate::ThemePick = crate::ThemePick::System;

/// The text color a scale's solid steps take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolidText {
    White,
    Dark,
    None,
}
/// scales.solid-text, per `Scale::ALL`.
pub(crate) const SOLID_TEXT: [SolidText; 25] = [SolidText::None, SolidText::None, SolidText::None, SolidText::None, SolidText::None, SolidText::White, SolidText::White, SolidText::White, SolidText::White, SolidText::White, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::None, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark, SolidText::Dark];

/// Radix 3.0.0, light and dark, per `Scale::ALL` and `GrayFamily::ALL`.
pub(crate) const SCALE_LIGHT: [[[u8; 3]; 12]; 25] = [
    // tomato
    [[255, 252, 252], [255, 248, 247], [254, 235, 231], [255, 220, 211], [255, 205, 194], [253, 189, 175], [245, 168, 152], [236, 142, 123], [229, 77, 46], [221, 68, 37], [209, 52, 21], [92, 39, 31]],
    // red
    [[255, 252, 252], [255, 247, 247], [254, 235, 236], [255, 219, 220], [255, 205, 206], [253, 189, 190], [244, 169, 170], [235, 142, 144], [229, 72, 77], [220, 62, 66], [206, 44, 49], [100, 23, 35]],
    // ruby
    [[255, 252, 253], [255, 247, 248], [254, 234, 237], [255, 220, 225], [255, 206, 214], [248, 191, 200], [239, 172, 184], [229, 146, 163], [229, 70, 102], [220, 59, 93], [202, 36, 77], [100, 23, 43]],
    // crimson
    [[255, 252, 253], [254, 247, 249], [255, 233, 240], [254, 220, 231], [250, 206, 221], [243, 190, 209], [234, 172, 195], [224, 147, 178], [233, 61, 130], [223, 52, 120], [203, 29, 99], [98, 22, 57]],
    // pink
    [[255, 252, 254], [254, 247, 251], [254, 233, 245], [251, 220, 239], [246, 206, 231], [239, 191, 221], [231, 172, 208], [221, 147, 194], [214, 64, 159], [207, 56, 151], [194, 41, 138], [101, 18, 73]],
    // plum
    [[254, 252, 255], [253, 247, 253], [251, 235, 251], [247, 222, 248], [242, 209, 243], [233, 194, 236], [222, 173, 227], [207, 145, 216], [171, 74, 186], [161, 68, 175], [149, 62, 163], [83, 25, 93]],
    // purple
    [[254, 252, 254], [251, 247, 254], [247, 237, 254], [242, 226, 252], [234, 213, 249], [224, 196, 244], [209, 175, 236], [190, 147, 228], [142, 78, 198], [131, 71, 185], [129, 69, 181], [64, 32, 96]],
    // violet
    [[253, 252, 254], [250, 248, 255], [244, 240, 254], [235, 228, 255], [225, 217, 255], [212, 202, 254], [194, 181, 245], [170, 153, 236], [110, 86, 207], [101, 77, 196], [101, 80, 185], [47, 38, 95]],
    // iris
    [[253, 253, 255], [248, 248, 255], [240, 241, 254], [230, 231, 255], [218, 220, 255], [203, 205, 255], [184, 186, 248], [155, 158, 240], [91, 91, 214], [81, 81, 205], [87, 83, 198], [39, 41, 98]],
    // indigo
    [[253, 253, 254], [247, 249, 255], [237, 242, 254], [225, 233, 255], [210, 222, 255], [193, 208, 255], [171, 189, 249], [141, 164, 239], [62, 99, 221], [51, 88, 212], [58, 91, 199], [31, 45, 92]],
    // blue
    [[251, 253, 255], [244, 250, 255], [230, 244, 254], [213, 239, 255], [194, 229, 255], [172, 216, 252], [142, 200, 246], [94, 177, 239], [0, 144, 255], [5, 136, 240], [13, 116, 206], [17, 50, 100]],
    // cyan
    [[250, 253, 254], [242, 250, 251], [222, 247, 249], [202, 241, 246], [181, 233, 240], [157, 221, 231], [125, 206, 220], [61, 185, 207], [0, 162, 199], [7, 151, 185], [16, 125, 152], [13, 60, 72]],
    // teal
    [[250, 254, 253], [243, 251, 249], [224, 248, 243], [204, 243, 234], [184, 234, 224], [161, 222, 210], [131, 205, 193], [83, 185, 171], [18, 165, 148], [13, 155, 138], [0, 133, 115], [13, 61, 56]],
    // jade
    [[251, 254, 253], [244, 251, 247], [230, 247, 237], [214, 241, 227], [195, 233, 215], [172, 222, 200], [139, 206, 182], [86, 186, 159], [41, 163, 131], [38, 153, 123], [32, 131, 104], [29, 59, 49]],
    // green
    [[251, 254, 252], [244, 251, 246], [230, 246, 235], [214, 241, 223], [196, 232, 209], [173, 221, 192], [142, 206, 170], [91, 185, 139], [48, 164, 108], [43, 154, 102], [33, 131, 88], [25, 59, 45]],
    // grass
    [[251, 254, 251], [245, 251, 245], [233, 246, 233], [218, 241, 219], [201, 232, 202], [178, 221, 181], [148, 206, 154], [101, 186, 116], [70, 167, 88], [62, 155, 79], [42, 126, 59], [32, 60, 37]],
    // brown
    [[254, 253, 252], [252, 249, 246], [246, 238, 231], [240, 228, 217], [235, 218, 202], [228, 205, 183], [220, 188, 159], [206, 163, 126], [173, 127, 88], [160, 117, 83], [129, 94, 70], [62, 51, 46]],
    // bronze
    [[253, 252, 252], [253, 247, 245], [246, 237, 234], [239, 228, 223], [231, 217, 211], [223, 205, 197], [211, 188, 179], [194, 164, 153], [161, 128, 114], [149, 116, 104], [125, 94, 84], [67, 48, 43]],
    // gold
    [[253, 253, 252], [250, 249, 242], [242, 240, 231], [234, 230, 219], [225, 220, 207], [216, 208, 191], [203, 192, 170], [185, 168, 141], [151, 131, 101], [140, 122, 94], [113, 98, 75], [59, 53, 43]],
    // sky
    [[249, 254, 255], [241, 250, 253], [225, 246, 253], [209, 240, 250], [190, 231, 245], [169, 218, 237], [141, 202, 227], [96, 179, 215], [124, 226, 254], [116, 218, 248], [0, 116, 158], [29, 62, 86]],
    // mint
    [[249, 254, 253], [242, 251, 249], [221, 249, 242], [200, 244, 233], [179, 236, 222], [156, 224, 208], [126, 207, 189], [76, 187, 165], [134, 234, 212], [125, 224, 203], [2, 120, 100], [22, 67, 60]],
    // lime
    [[252, 253, 250], [248, 250, 243], [238, 246, 214], [226, 240, 189], [211, 231, 166], [194, 218, 145], [171, 201, 120], [141, 182, 84], [189, 238, 99], [176, 230, 76], [92, 124, 47], [55, 64, 28]],
    // yellow
    [[253, 253, 249], [254, 252, 233], [255, 250, 184], [255, 243, 148], [255, 231, 112], [243, 215, 104], [228, 199, 103], [213, 174, 57], [255, 230, 41], [255, 220, 0], [158, 108, 0], [71, 59, 31]],
    // amber
    [[254, 253, 251], [254, 251, 233], [255, 247, 194], [255, 238, 156], [251, 229, 119], [243, 214, 115], [233, 193, 98], [226, 163, 54], [255, 197, 61], [255, 186, 24], [171, 100, 0], [79, 52, 34]],
    // orange
    [[254, 252, 251], [255, 247, 237], [255, 239, 214], [255, 223, 181], [255, 209, 154], [255, 193, 130], [245, 174, 115], [236, 148, 85], [247, 107, 21], [239, 95, 0], [204, 78, 0], [88, 45, 29]],
];
pub(crate) const SCALE_DARK: [[[u8; 3]; 12]; 25] = [
    // tomato
    [[24, 17, 17], [31, 21, 19], [57, 23, 20], [78, 21, 17], [94, 28, 22], [110, 41, 32], [133, 58, 45], [172, 77, 57], [229, 77, 46], [236, 97, 66], [255, 151, 125], [251, 211, 203]],
    // red
    [[25, 17, 17], [32, 19, 20], [59, 18, 25], [80, 15, 28], [97, 22, 35], [114, 35, 45], [140, 51, 58], [181, 69, 72], [229, 72, 77], [236, 93, 94], [255, 149, 146], [255, 209, 217]],
    // ruby
    [[25, 17, 19], [30, 21, 23], [58, 20, 30], [78, 19, 37], [94, 26, 46], [111, 37, 57], [136, 52, 71], [179, 68, 90], [229, 70, 102], [236, 90, 114], [255, 148, 157], [254, 210, 225]],
    // crimson
    [[25, 17, 20], [32, 19, 24], [56, 21, 37], [77, 18, 47], [92, 24, 57], [109, 37, 69], [135, 51, 86], [176, 67, 110], [233, 61, 130], [238, 81, 138], [255, 146, 173], [253, 211, 232]],
    // pink
    [[25, 17, 23], [33, 18, 29], [55, 23, 47], [75, 20, 61], [89, 28, 71], [105, 41, 85], [131, 56, 105], [168, 72, 133], [214, 64, 159], [222, 81, 168], [255, 141, 204], [253, 209, 234]],
    // plum
    [[24, 17, 24], [32, 19, 32], [53, 26, 53], [69, 29, 71], [81, 36, 84], [94, 48, 97], [115, 64, 121], [146, 84, 156], [171, 74, 186], [182, 88, 196], [231, 150, 243], [244, 212, 244]],
    // purple
    [[24, 17, 27], [30, 21, 35], [48, 28, 59], [61, 34, 78], [72, 41, 92], [84, 52, 107], [102, 66, 130], [132, 87, 170], [142, 78, 198], [154, 92, 208], [209, 157, 255], [236, 217, 250]],
    // violet
    [[20, 18, 31], [27, 21, 37], [41, 31, 67], [51, 37, 91], [60, 46, 105], [71, 56, 118], [86, 70, 139], [105, 88, 173], [110, 86, 207], [125, 102, 217], [186, 167, 255], [226, 221, 254]],
    // iris
    [[19, 19, 30], [23, 22, 37], [32, 34, 72], [38, 42, 101], [48, 51, 116], [61, 62, 130], [74, 74, 149], [89, 88, 177], [91, 91, 214], [110, 106, 222], [177, 169, 255], [224, 223, 254]],
    // indigo
    [[17, 19, 31], [20, 23, 38], [24, 36, 73], [29, 46, 98], [37, 57, 116], [48, 67, 132], [58, 79, 151], [67, 93, 177], [62, 99, 221], [84, 114, 228], [158, 177, 255], [214, 225, 255]],
    // blue
    [[13, 21, 32], [17, 25, 39], [13, 40, 71], [0, 51, 98], [0, 64, 116], [16, 77, 135], [32, 93, 158], [40, 112, 189], [0, 144, 255], [59, 158, 255], [112, 184, 255], [194, 230, 255]],
    // cyan
    [[11, 22, 26], [16, 27, 32], [8, 44, 54], [0, 56, 72], [0, 69, 88], [4, 84, 104], [18, 103, 126], [17, 128, 156], [0, 162, 199], [35, 175, 208], [76, 204, 230], [182, 236, 247]],
    // teal
    [[13, 21, 20], [17, 28, 27], [13, 45, 42], [2, 59, 55], [8, 72, 67], [20, 87, 80], [28, 105, 97], [32, 126, 115], [18, 165, 148], [14, 179, 158], [11, 216, 182], [173, 240, 221]],
    // jade
    [[13, 21, 18], [18, 28, 24], [15, 46, 34], [11, 59, 44], [17, 72, 55], [27, 87, 69], [36, 104, 84], [42, 126, 104], [41, 163, 131], [39, 176, 139], [31, 216, 164], [173, 240, 212]],
    // green
    [[14, 21, 18], [18, 27, 23], [19, 45, 33], [17, 59, 41], [23, 73, 51], [32, 87, 62], [40, 104, 74], [47, 124, 87], [48, 164, 108], [51, 176, 116], [61, 214, 140], [177, 241, 203]],
    // grass
    [[14, 21, 17], [20, 26, 21], [27, 42, 30], [29, 58, 36], [37, 72, 45], [45, 87, 54], [54, 103, 64], [62, 121, 73], [70, 167, 88], [83, 179, 101], [113, 208, 131], [194, 240, 194]],
    // brown
    [[18, 17, 15], [28, 24, 22], [40, 33, 29], [50, 41, 34], [62, 49, 40], [77, 60, 47], [97, 74, 57], [124, 95, 70], [173, 127, 88], [184, 140, 103], [219, 181, 148], [242, 225, 202]],
    // bronze
    [[20, 17, 16], [28, 25, 23], [38, 34, 32], [48, 42, 39], [59, 51, 48], [73, 62, 58], [90, 76, 71], [111, 95, 88], [161, 128, 114], [174, 140, 126], [212, 179, 165], [237, 224, 217]],
    // gold
    [[18, 18, 17], [27, 26, 23], [36, 35, 31], [45, 43, 38], [56, 53, 46], [68, 64, 57], [84, 79, 70], [105, 98, 86], [151, 131, 101], [163, 144, 115], [203, 185, 159], [232, 226, 217]],
    // sky
    [[13, 20, 31], [17, 26, 39], [17, 40, 64], [17, 53, 85], [21, 68, 103], [27, 83, 123], [31, 102, 146], [25, 124, 174], [124, 226, 254], [168, 238, 255], [117, 199, 240], [194, 243, 255]],
    // mint
    [[14, 21, 21], [15, 27, 27], [9, 44, 43], [0, 58, 56], [0, 71, 68], [16, 86, 80], [30, 104, 95], [39, 127, 112], [134, 234, 212], [168, 245, 229], [88, 213, 186], [196, 245, 225]],
    // lime
    [[17, 19, 12], [21, 26, 16], [31, 41, 23], [41, 55, 29], [51, 68, 35], [61, 82, 42], [73, 98, 49], [87, 117, 56], [189, 238, 99], [212, 255, 112], [189, 229, 108], [227, 247, 186]],
    // yellow
    [[20, 18, 11], [27, 24, 15], [45, 35, 5], [54, 43, 0], [67, 53, 0], [82, 66, 2], [102, 84, 23], [131, 106, 33], [255, 230, 41], [255, 255, 87], [245, 225, 71], [246, 238, 180]],
    // amber
    [[22, 18, 12], [29, 24, 15], [48, 32, 8], [63, 39, 0], [77, 48, 0], [92, 61, 5], [113, 79, 25], [143, 100, 36], [255, 197, 61], [255, 214, 10], [255, 202, 22], [255, 231, 179]],
    // orange
    [[23, 18, 14], [30, 22, 15], [51, 30, 11], [70, 33, 0], [86, 40, 0], [102, 53, 12], [126, 69, 29], [163, 88, 41], [247, 107, 21], [255, 128, 31], [255, 160, 87], [255, 224, 194]],
];
pub(crate) const GRAY_LIGHT: [[[u8; 3]; 12]; 6] = [
    // gray
    [[252, 252, 252], [249, 249, 249], [240, 240, 240], [232, 232, 232], [224, 224, 224], [217, 217, 217], [206, 206, 206], [187, 187, 187], [141, 141, 141], [131, 131, 131], [100, 100, 100], [32, 32, 32]],
    // mauve
    [[253, 252, 253], [250, 249, 251], [242, 239, 243], [234, 231, 236], [227, 223, 230], [219, 216, 224], [208, 205, 215], [188, 186, 199], [142, 140, 153], [132, 130, 142], [101, 99, 109], [33, 31, 38]],
    // slate
    [[252, 252, 253], [249, 249, 251], [240, 240, 243], [232, 232, 236], [224, 225, 230], [217, 217, 224], [205, 206, 214], [185, 187, 198], [139, 141, 152], [128, 131, 141], [96, 100, 108], [28, 32, 36]],
    // sage
    [[251, 253, 252], [247, 249, 248], [238, 241, 240], [230, 233, 232], [223, 226, 224], [215, 218, 217], [203, 207, 205], [184, 188, 186], [134, 142, 139], [124, 132, 129], [95, 101, 99], [26, 33, 30]],
    // olive
    [[252, 253, 252], [248, 250, 248], [239, 241, 239], [231, 233, 231], [223, 226, 223], [215, 218, 215], [204, 207, 204], [185, 188, 184], [137, 142, 135], [127, 132, 125], [96, 101, 95], [29, 33, 28]],
    // sand
    [[253, 253, 252], [249, 249, 248], [241, 240, 239], [233, 232, 230], [226, 225, 222], [218, 217, 214], [207, 206, 202], [188, 187, 181], [141, 141, 134], [130, 130, 124], [99, 99, 94], [33, 32, 28]],
];
pub(crate) const GRAY_DARK: [[[u8; 3]; 12]; 6] = [
    // gray
    [[17, 17, 17], [25, 25, 25], [34, 34, 34], [42, 42, 42], [49, 49, 49], [58, 58, 58], [72, 72, 72], [96, 96, 96], [110, 110, 110], [123, 123, 123], [180, 180, 180], [238, 238, 238]],
    // mauve
    [[18, 17, 19], [26, 25, 27], [35, 34, 37], [43, 41, 45], [50, 48, 53], [60, 57, 63], [73, 71, 78], [98, 95, 105], [111, 109, 120], [124, 122, 133], [181, 178, 188], [238, 238, 240]],
    // slate
    [[17, 17, 19], [24, 25, 27], [33, 34, 37], [39, 42, 45], [46, 49, 53], [54, 58, 63], [67, 72, 78], [90, 97, 105], [105, 110, 119], [119, 123, 132], [176, 180, 186], [237, 238, 240]],
    // sage
    [[16, 18, 17], [23, 25, 24], [32, 34, 33], [39, 42, 41], [46, 49, 48], [55, 59, 57], [68, 73, 71], [91, 98, 95], [99, 112, 107], [113, 125, 121], [173, 181, 178], [236, 238, 237]],
    // olive
    [[17, 18, 16], [24, 25, 23], [33, 34, 32], [40, 42, 39], [47, 49, 46], [56, 58, 54], [69, 72, 67], [92, 98, 91], [104, 112, 102], [118, 125, 116], [175, 181, 173], [236, 238, 236]],
    // sand
    [[17, 17, 16], [25, 25, 24], [34, 34, 33], [42, 42, 40], [49, 49, 46], [59, 58, 55], [73, 72, 68], [98, 96, 91], [111, 109, 102], [124, 123, 116], [181, 179, 173], [238, 238, 236]],
];
/// blackA1-12: black at these opacities.
pub(crate) const BLACK_A: [f64; 12] = [0.05, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95];

/// Which pick or fixed scale a role reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Gray,
    Widget,
    Host,
    Ok,
    Warning,
    Error,
    Overlay,
}

/// The step a role takes in one theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepRef {
    Radix(u8),
    White,
    Black,
    SolidText,
}

/// A color role: where it reads its color in each theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RoleDef {
    pub(crate) name: &'static str,
    pub(crate) source: Source,
    pub(crate) light: StepRef,
    pub(crate) dark: StepRef,
}

pub(crate) const ROLES: &[RoleDef] = &[
    // roles.text: Body text; step 12 is Radix's high-contrast text.
    RoleDef { name: "text", source: Source::Gray, light: StepRef::Radix(12), dark: StepRef::Radix(12) },
    // roles.text-secondary: Labels, captions and the field note; step 11 is Radix's low-contrast text, still AA on steps 1-3.
    RoleDef { name: "text-secondary", source: Source::Gray, light: StepRef::Radix(11), dark: StepRef::Radix(11) },
    // roles.accent-text: Accented text on a plain surface, such as a link in widget content.
    RoleDef { name: "accent-text", source: Source::Widget, light: StepRef::Radix(11), dark: StepRef::Radix(11) },
    // roles.accent-strong: Text on a soft accent; step 11 on step 3 of the same scale is under 4.5:1 for some scales.
    RoleDef { name: "accent-strong", source: Source::Widget, light: StepRef::Radix(12), dark: StepRef::Radix(12) },
    // roles.border-control: A control's boundary identifies it and needs 3:1 (WCAG 1.4.11); Radix's border steps 6-8 fall short.
    RoleDef { name: "border-control", source: Source::Gray, light: StepRef::Radix(9), dark: StepRef::Radix(9) },
    // roles.divider: Radix's subtle border step, for lines between rows and around the chrome.
    RoleDef { name: "divider", source: Source::Gray, light: StepRef::Radix(6), dark: StepRef::Radix(6) },
    // roles.ok-icon: The status icon carries the status beside gray text, so it needs 3:1 (WCAG 1.4.11).
    RoleDef { name: "ok-icon", source: Source::Ok, light: StepRef::Radix(11), dark: StepRef::Radix(11) },
    // roles.warning-icon: As ok-icon; amber 9 is too light to carry meaning on a light surface.
    RoleDef { name: "warning-icon", source: Source::Warning, light: StepRef::Radix(11), dark: StepRef::Radix(11) },
    // roles.error-icon: As ok-icon.
    RoleDef { name: "error-icon", source: Source::Error, light: StepRef::Radix(11), dark: StepRef::Radix(11) },
    // roles.canvas: The window behind the frames; Radix's app background steps.
    RoleDef { name: "canvas", source: Source::Gray, light: StepRef::Radix(2), dark: StepRef::Radix(1) },
    // roles.surface: A widget's frame and the fields drawn in it, one step off the canvas so frames stand out.
    RoleDef { name: "surface", source: Source::Gray, light: StepRef::Radix(1), dark: StepRef::Radix(2) },
    // roles.neutral-soft: Buttons, the monogram and badges: Radix's component background step.
    RoleDef { name: "neutral-soft", source: Source::Gray, light: StepRef::Radix(3), dark: StepRef::Radix(3) },
    // roles.accent-soft: An accented button or badge: the widget color's component background step.
    RoleDef { name: "accent-soft", source: Source::Widget, light: StepRef::Radix(3), dark: StepRef::Radix(3) },
    // roles.ok-soft: The background of an ok badge.
    RoleDef { name: "ok-soft", source: Source::Ok, light: StepRef::Radix(3), dark: StepRef::Radix(3) },
    // roles.warning-soft: The background of a warning badge.
    RoleDef { name: "warning-soft", source: Source::Warning, light: StepRef::Radix(3), dark: StepRef::Radix(3) },
    // roles.error-soft: The background of an error badge.
    RoleDef { name: "error-soft", source: Source::Error, light: StepRef::Radix(3), dark: StepRef::Radix(3) },
];

/// The built-in status icons, SVG path data on a 16-unit grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Check,
    Alert,
    Cross,
}

impl Icon {
    /// The icon's SVG path data, drawn with the even-odd rule on a 16 × 16 grid.
    pub const fn path(self) -> &'static str {
        match self {
            Icon::Check => "M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm3.35 4.65-4.1 4.6a.75.75 0 0 1-1.1.03L4.4 8.53a.75.75 0 1 1 1.06-1.06l1.2 1.2 3.57-4.02a.75.75 0 1 1 1.12 1Z",
            Icon::Alert => "M8.87 1.5a1 1 0 0 0-1.74 0L.63 13a1 1 0 0 0 .87 1.5h13a1 1 0 0 0 .87-1.5L8.87 1.5ZM8 5a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 8 5Zm0 7.5a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z",
            Icon::Cross => "M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm2.53 4.47a.75.75 0 0 1 0 1.06L9.06 8l1.47 1.47a.75.75 0 1 1-1.06 1.06L8 9.06l-1.47 1.47a.75.75 0 0 1-1.06-1.06L6.94 8 5.47 6.53a.75.75 0 0 1 1.06-1.06L8 6.94l1.47-1.47a.75.75 0 0 1 1.06 0Z",
        }
    }
}

/// A tone as roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToneDef {
    pub(crate) text: &'static str,
    pub(crate) surface: &'static str,
    pub(crate) border: Option<&'static str>,
    pub(crate) icon: Option<&'static str>,
    pub(crate) status_icon: Option<Icon>,
}

/// tones: default, accent, ok, warning, error.
pub(crate) const TONES: [ToneDef; 5] = [
    // default
    ToneDef { text: "text", surface: "surface", border: Some("border-control"), icon: None, status_icon: None },
    // accent
    ToneDef { text: "accent-strong", surface: "accent-soft", border: None, icon: None, status_icon: None },
    // ok
    ToneDef { text: "text", surface: "ok-soft", border: None, icon: Some("ok-icon"), status_icon: Some(Icon::Check) },
    // warning
    ToneDef { text: "text", surface: "warning-soft", border: None, icon: Some("warning-icon"), status_icon: Some(Icon::Alert) },
    // error
    ToneDef { text: "text", surface: "error-soft", border: None, icon: Some("error-icon"), status_icon: Some(Icon::Cross) },
];

/// The type scale; sizes in pixels.
pub mod typography {
    /// A type level.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Level {
        pub family: &'static str,
        pub size: f32,
        pub weight: u16,
        pub line_height: f32,
        pub letter_spacing_em: f32,
    }

    /// typography.caption: Timestamps and the field note: the smallest text, regular weight.
    pub const CAPTION: Level = Level { family: "Inter", size: 12.0, weight: 400, line_height: 16.0, letter_spacing_em: 0.0 };
    /// typography.label-sm: Frame labels, badges and the monogram: small but medium weight so it holds.
    pub const LABEL_SM: Level = Level { family: "Inter", size: 12.0, weight: 500, line_height: 16.0, letter_spacing_em: 0.0 };
    /// typography.body-sm: Dense secondary content inside widgets.
    pub const BODY_SM: Level = Level { family: "Inter", size: 13.0, weight: 400, line_height: 18.0, letter_spacing_em: 0.0 };
    /// typography.body-md: Body text and fields: 14 px reads well at desktop distance and fits dense widgets.
    pub const BODY_MD: Level = Level { family: "Inter", size: 14.0, weight: 400, line_height: 20.0, letter_spacing_em: 0.0 };
    /// typography.label-md: Buttons and the chrome: body size, medium weight.
    pub const LABEL_MD: Level = Level { family: "Inter", size: 14.0, weight: 500, line_height: 20.0, letter_spacing_em: 0.0 };
    /// typography.title-sm: A widget's own heading.
    pub const TITLE_SM: Level = Level { family: "Inter", size: 16.0, weight: 600, line_height: 24.0, letter_spacing_em: 0.0 };
    /// typography.title-md: Panel and dialog titles.
    pub const TITLE_MD: Level = Level { family: "Inter", size: 20.0, weight: 600, line_height: 28.0, letter_spacing_em: -0.01 };
    /// typography.title-lg: Large numbers and page titles.
    pub const TITLE_LG: Level = Level { family: "Inter", size: 24.0, weight: 600, line_height: 32.0, letter_spacing_em: -0.015 };
    /// typography.display: The largest figure a widget shows, such as a timer.
    pub const DISPLAY: Level = Level { family: "Inter", size: 32.0, weight: 600, line_height: 40.0, letter_spacing_em: -0.02 };
}

/// The spacing scale, in pixels.
pub mod spacing {
    /// spacing.none: Flush.
    pub const NONE: f32 = 0.0;
    /// spacing.xxs: Between an icon and its label.
    pub const XXS: f32 = 2.0;
    /// spacing.xs: Inside compact controls.
    pub const XS: f32 = 4.0;
    /// spacing.sm: Between related items, and a field's padding.
    pub const SM: f32 = 8.0;
    /// spacing.md: A frame's padding and the grid's gutter.
    pub const MD: f32 = 12.0;
    /// spacing.lg: A panel's padding.
    pub const LG: f32 = 16.0;
    /// spacing.xl: A dialog's padding.
    pub const XL: f32 = 24.0;
    /// spacing.2xl: Between groups in a dialog.
    pub const XL2: f32 = 32.0;
    /// spacing.3xl: Around empty states.
    pub const XL3: f32 = 48.0;
}

/// The corner radii, in pixels; the person picks one of them.
pub mod radius {
    /// rounded.none: A pick for square corners.
    pub const NONE: f32 = 0.0;
    /// rounded.small: The monogram, and corners when the person picks small.
    pub const SMALL: f32 = 3.0;
    /// rounded.medium: The default: frames, buttons and fields.
    pub const MEDIUM: f32 = 6.0;
    /// rounded.large: Dialogs, a step rounder than what they float over.
    pub const LARGE: f32 = 10.0;
    /// rounded.full: Pills: badges, and every corner when the person picks full.
    pub const FULL: f32 = 9999.0;
}

/// radius-picks.
pub(crate) const RADIUS_PICKS: [&str; 5] = ["none", "small", "medium", "large", "full"];

/// Elevation levels the kit may use.
pub mod elevation {
/// A shadow: offsets and blur in pixels, its color a step of Radix's blackA.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Shadow {
        pub x: f32,
        pub y: f32,
        pub blur: f32,
        pub black_a: u8,
    }

    /// elevation.flat: Most content sits flat on its frame.
    pub const FLAT: Option<Shadow> = None;
    /// elevation.frame: A hint of lift so a frame reads as one object on the canvas.
    pub const FRAME: Option<Shadow> = Some(Shadow { x: 0.0, y: 1.0, blur: 2.0, black_a: 2 });
}
