#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AndroidKeys {
    Unknown = 0,
    SoftLeft = 1,
    SoftRight = 2,
    Home = 3,
    Back = 4,
    Call = 5,
    EndCall = 6,
    Key0 = 7,
    Key1 = 8,
    Key2 = 9,
    Key3 = 10,
    Key4 = 11,
    Key5 = 12,
    Key6 = 13,
    Key7 = 14,
    Key8 = 15,
    Key9 = 16,
    Star = 17,
    Pound = 18,
    DpadUp = 19,
    DpadDown = 20,
    DpadLeft = 21,
    DpadRight = 22,
    DpadCenter = 23,
    VolumeUp = 24,
    VolumeDown = 25,
    Power = 26,
    Camera = 27,
    Clear = 28,
    A = 29,
    B = 30,
    C = 31,
    D = 32,
    E = 33,
    F = 34,
    G = 35,
    H = 36,
    I = 37,
    J = 38,
    K = 39,
    L = 40,
    M = 41,
    N = 42,
    O = 43,
    P = 44,
    Q = 45,
    R = 46,
    S = 47,
    T = 48,
    U = 49,
    V = 50,
    W = 51,
    X = 52,
    Y = 53,
    Z = 54,
    Comma = 55,
    Period = 56,
    AltLeft = 57,
    AltRight = 58,
    ShiftLeft = 59,
    ShiftRight = 60,
    Tab = 61,
    Space = 62,
    Sym = 63,
    Explorer = 64,
    Envelope = 65,
    Enter = 66,
    Delete = 67,
    Grave = 68,
    Minus = 69,
    Equals = 70,
    LeftBracket = 71,
    RightBracket = 72,
    Backslash = 73,
    Semicolon = 74,
    Apostrophe = 75,
    Slash = 76,
    At = 77,
    Num = 78,
    HeadsetHook = 79,
    Focus = 80,
    Plus = 81,
    Menu = 82,
    Notification = 83,
    Search = 84,
    MediaPlayPause = 85,
    MediaStop = 86,
    MediaNext = 87,
    MediaPrevious = 88,
    MediaRewind = 89,
    MediaFastForward = 90,
    Mute = 91,
    PageUp = 92,
    PageDown = 93,
    Pictsymbols = 94,
    SwitchCharset = 95,
    ButtonA = 96,
    ButtonB = 97,
    ButtonC = 98,
    ButtonX = 99,
    ButtonY = 100,
    ButtonZ = 101,
    ButtonL1 = 102,
    ButtonR1 = 103,
    ButtonL2 = 104,
    ButtonR2 = 105,
    ButtonThumbl = 106,
    ButtonThumbr = 107,
    ButtonStart = 108,
    ButtonSelect = 109,
    ButtonMode = 110,
    Escape = 111,
    ForwardDel = 112,
    ControlLeft = 113,
    ControlRight = 114,
    CapsLock = 115,
    ScrollLock = 116,
    MetaLeft = 117,
    MetaRight = 118,
    Function = 119,
    SysRq = 120,
    Break = 121,
    MoveHome = 122,
    MoveEnd = 123,
    Insert = 124,
    Forward = 125,
    MediaPlay = 126,
    MediaPause = 127,
    MediaClose = 128,
    MediaEject = 129,
    MediaRecord = 130,
    F1 = 131,
    F2 = 132,
    F3 = 133,
    F4 = 134,
    F5 = 135,
    F6 = 136,
    F7 = 137,
    F8 = 138,
    F9 = 139,
    F10 = 140,
    F11 = 141,
    F12 = 142,
    NumLock = 143,
    Numpad0 = 144,
    Numpad1 = 145,
    Numpad2 = 146,
    Numpad3 = 147,
    Numpad4 = 148,
    Numpad5 = 149,
    Numpad6 = 150,
    Numpad7 = 151,
    Numpad8 = 152,
    Numpad9 = 153,
    NumpadDivide = 154,
    NumpadMultiply = 155,
    NumpadSubtract = 156,
    NumpadAdd = 157,
    NumpadDot = 158,
    NumpadComma = 159,
    NumpadEnter = 160,
    NumpadEquals = 161,
    NumpadLeftParen = 162,
    NumpadRightParen = 163,
    VolumeMute = 164,
    Info = 165,
    ChannelUp = 166,
    ChannelDown = 167,
    ZoomIn = 168,
    ZoomOut = 169,
    Tv = 170,
    Window = 171,
    Guide = 172,
    Dvr = 173,
    Bookmark = 174,
    Captions = 175,
    Settings = 176,
    TvPower = 177,
    TvInput = 178,
    StbPower = 179,
    StbInput = 180,
    AvrPower = 181,
    AvrInput = 182,
    ProgRed = 183,
    ProgGreen = 184,
    ProgYellow = 185,
    ProgBlue = 186,
    AppSwitch = 187,
    Button1 = 188,
    Button2 = 189,
    Button3 = 190,
    Button4 = 191,
    Button5 = 192,
    Button6 = 193,
    Button7 = 194,
    Button8 = 195,
    Button9 = 196,
    Button10 = 197,
    Button11 = 198,
    Button12 = 199,
    Button13 = 200,
    Button14 = 201,
    Button15 = 202,
    Button16 = 203,
    LanguageSwitch = 204,
    MannerMode = 205,
    ThreeDMode = 206,
    Contacts = 207,
    Calendar = 208,
    Music = 209,
    Calculator = 210,
    ZenkakuHankaku = 211,
    Eisu = 212,
    Muhenkan = 213,
    Henkan = 214,
    KatakanaHiragana = 215,
    Yen = 216,
    Ro = 217,
    Kana = 218,
    Assist = 219,
    BrightnessDown = 220,
    BrightnessUp = 221,
    MediaAudioTrack = 222,
    Sleep = 223,
    Wakeup = 224,
    Pairing = 225,
    MediaTopMenu = 226,
    Key11 = 227,
    Key12 = 228,
    LastChannel = 229,
    TvDataService = 230,
    VoiceAssist = 231,
    TvRadioService = 232,
    TvTeletext = 233,
    TvNumberEntry = 234,
    TvTerrestrialAnalog = 235,
    TvTerrestrialDigital = 236,
    TvSatellite = 237,
    TvSatelliteBs = 238,
    TvSatelliteCs = 239,
    TvSatelliteService = 240,
    TvNetwork = 241,
    TvAntennaCable = 242,
    TvInputHdmi1 = 243,
    TvInputHdmi2 = 244,
    TvInputHdmi3 = 245,
    TvInputHdmi4 = 246,
    TvInputComposite1 = 247,
    TvInputComposite2 = 248,
    TvInputComponent1 = 249,
    TvInputComponent2 = 250,
    TvInputVga1 = 251,
    TvAudioDescription = 252,
    TvAudioDescriptionMixUp = 253,
    TvAudioDescriptionMixDown = 254,
    TvZoomMode = 255,
    TvContentsMenu = 256,
    TvMediaContextMenu = 257,
    TvTimerProgramming = 258,
    Help = 259,
    NavigatePrevious = 260,
    NavigateNext = 261,
    NavigateIn = 262,
    NavigateOut = 263,
    StemPrimary = 264,
    Stem1 = 265,
    Stem2 = 266,
    Stem3 = 267,
    DpadUpLeft = 268,
    DpadDownLeft = 269,
    DpadUpRight = 270,
    DpadDownRight = 271,
    MediaSkipForward = 272,
    MediaSkipBackward = 273,
    MediaStepForward = 274,
    MediaStepBackward = 275,
    SoftSleep = 276,
    Cut = 277,
    Copy = 278,
    Paste = 279,
    SystemNavigationUp = 280,
    SystemNavigationDown = 281,
    SystemNavigationLeft = 282,
    SystemNavigationRight = 283,
    AllApps = 284,
    ThumbsUp = 285,
    ThumbsDown = 286,
    ProfileSwitch = 287,
}

impl AndroidKeys {
    /// sourced from here:
    /// https://developer.android.com/reference/android/view/KeyEvent
    pub fn from(key_code: i32) -> Option<Self> {
        use AndroidKeys::*;
        match key_code {
            0 => Some(Unknown),
            1 => Some(SoftLeft),
            2 => Some(SoftRight),
            3 => Some(Home),
            4 => Some(Back),
            5 => Some(Call),
            6 => Some(EndCall),
            7 => Some(Key0),
            8 => Some(Key1),
            9 => Some(Key2),
            10 => Some(Key3),
            11 => Some(Key4),
            12 => Some(Key5),
            13 => Some(Key6),
            14 => Some(Key7),
            15 => Some(Key8),
            16 => Some(Key9),
            17 => Some(Star),
            18 => Some(Pound),
            19 => Some(DpadUp),
            20 => Some(DpadDown),
            21 => Some(DpadLeft),
            22 => Some(DpadRight),
            23 => Some(DpadCenter),
            24 => Some(VolumeUp),
            25 => Some(VolumeDown),
            26 => Some(Power),
            27 => Some(Camera),
            28 => Some(Clear),
            29 => Some(A),
            30 => Some(B),
            31 => Some(C),
            32 => Some(D),
            33 => Some(E),
            34 => Some(F),
            35 => Some(G),
            36 => Some(H),
            37 => Some(I),
            38 => Some(J),
            39 => Some(K),
            40 => Some(L),
            41 => Some(M),
            42 => Some(N),
            43 => Some(O),
            44 => Some(P),
            45 => Some(Q),
            46 => Some(R),
            47 => Some(S),
            48 => Some(T),
            49 => Some(U),
            50 => Some(V),
            51 => Some(W),
            52 => Some(X),
            53 => Some(Y),
            54 => Some(Z),
            55 => Some(Comma),
            56 => Some(Period),
            57 => Some(AltLeft),
            58 => Some(AltRight),
            59 => Some(ShiftLeft),
            60 => Some(ShiftRight),
            61 => Some(Tab),
            62 => Some(Space),
            63 => Some(Sym),
            64 => Some(Explorer),
            65 => Some(Envelope),
            66 => Some(Enter),
            67 => Some(Delete),
            68 => Some(Grave),
            69 => Some(Minus),
            70 => Some(Equals),
            71 => Some(LeftBracket),
            72 => Some(RightBracket),
            73 => Some(Backslash),
            74 => Some(Semicolon),
            75 => Some(Apostrophe),
            76 => Some(Slash),
            77 => Some(At),
            78 => Some(Num),
            79 => Some(HeadsetHook),
            80 => Some(Focus),
            81 => Some(Plus),
            82 => Some(Menu),
            83 => Some(Notification),
            84 => Some(Search),
            85 => Some(MediaPlayPause),
            86 => Some(MediaStop),
            87 => Some(MediaNext),
            88 => Some(MediaPrevious),
            89 => Some(MediaRewind),
            90 => Some(MediaFastForward),
            91 => Some(Mute),
            92 => Some(PageUp),
            93 => Some(PageDown),
            94 => Some(Pictsymbols),
            95 => Some(SwitchCharset),
            96 => Some(ButtonA),
            97 => Some(ButtonB),
            98 => Some(ButtonC),
            99 => Some(ButtonX),
            100 => Some(ButtonY),
            101 => Some(ButtonZ),
            102 => Some(ButtonL1),
            103 => Some(ButtonR1),
            104 => Some(ButtonL2),
            105 => Some(ButtonR2),
            106 => Some(ButtonThumbl),
            107 => Some(ButtonThumbr),
            108 => Some(ButtonStart),
            109 => Some(ButtonSelect),
            110 => Some(ButtonMode),
            111 => Some(Escape),
            112 => Some(ForwardDel),
            113 => Some(ControlLeft),
            114 => Some(ControlRight),
            115 => Some(CapsLock),
            116 => Some(ScrollLock),
            117 => Some(MetaLeft),
            118 => Some(MetaRight),
            119 => Some(Function),
            120 => Some(SysRq),
            121 => Some(Break),
            122 => Some(MoveHome),
            123 => Some(MoveEnd),
            124 => Some(Insert),
            125 => Some(Forward),
            126 => Some(MediaPlay),
            127 => Some(MediaPause),
            128 => Some(MediaClose),
            129 => Some(MediaEject),
            130 => Some(MediaRecord),
            131 => Some(F1),
            132 => Some(F2),
            133 => Some(F3),
            134 => Some(F4),
            135 => Some(F5),
            136 => Some(F6),
            137 => Some(F7),
            138 => Some(F8),
            139 => Some(F9),
            140 => Some(F10),
            141 => Some(F11),
            142 => Some(F12),
            143 => Some(NumLock),
            144 => Some(Numpad0),
            145 => Some(Numpad1),
            146 => Some(Numpad2),
            147 => Some(Numpad3),
            148 => Some(Numpad4),
            149 => Some(Numpad5),
            150 => Some(Numpad6),
            151 => Some(Numpad7),
            152 => Some(Numpad8),
            153 => Some(Numpad9),
            154 => Some(NumpadDivide),
            155 => Some(NumpadMultiply),
            156 => Some(NumpadSubtract),
            157 => Some(NumpadAdd),
            158 => Some(NumpadDot),
            159 => Some(NumpadComma),
            160 => Some(NumpadEnter),
            161 => Some(NumpadEquals),
            162 => Some(NumpadLeftParen),
            163 => Some(NumpadRightParen),
            164 => Some(VolumeMute),
            165 => Some(Info),
            166 => Some(ChannelUp),
            167 => Some(ChannelDown),
            168 => Some(ZoomIn),
            169 => Some(ZoomOut),
            170 => Some(Tv),
            171 => Some(Window),
            172 => Some(Guide),
            173 => Some(Dvr),
            174 => Some(Bookmark),
            175 => Some(Captions),
            176 => Some(Settings),
            177 => Some(TvPower),
            178 => Some(TvInput),
            179 => Some(StbPower),
            180 => Some(StbInput),
            181 => Some(AvrPower),
            182 => Some(AvrInput),
            183 => Some(ProgRed),
            184 => Some(ProgGreen),
            185 => Some(ProgYellow),
            186 => Some(ProgBlue),
            187 => Some(AppSwitch),
            188 => Some(Button1),
            189 => Some(Button2),
            190 => Some(Button3),
            191 => Some(Button4),
            192 => Some(Button5),
            193 => Some(Button6),
            194 => Some(Button7),
            195 => Some(Button8),
            196 => Some(Button9),
            197 => Some(Button10),
            198 => Some(Button11),
            199 => Some(Button12),
            200 => Some(Button13),
            201 => Some(Button14),
            202 => Some(Button15),
            203 => Some(Button16),
            204 => Some(LanguageSwitch),
            205 => Some(MannerMode),
            206 => Some(ThreeDMode),
            207 => Some(Contacts),
            208 => Some(Calendar),
            209 => Some(Music),
            210 => Some(Calculator),
            211 => Some(ZenkakuHankaku),
            212 => Some(Eisu),
            213 => Some(Muhenkan),
            214 => Some(Henkan),
            215 => Some(KatakanaHiragana),
            216 => Some(Yen),
            217 => Some(Ro),
            218 => Some(Kana),
            219 => Some(Assist),
            220 => Some(BrightnessDown),
            221 => Some(BrightnessUp),
            222 => Some(MediaAudioTrack),
            223 => Some(Sleep),
            224 => Some(Wakeup),
            225 => Some(Pairing),
            226 => Some(MediaTopMenu),
            227 => Some(Key11),
            228 => Some(Key12),
            229 => Some(LastChannel),
            230 => Some(TvDataService),
            231 => Some(VoiceAssist),
            232 => Some(TvRadioService),
            233 => Some(TvTeletext),
            234 => Some(TvNumberEntry),
            235 => Some(TvTerrestrialAnalog),
            236 => Some(TvTerrestrialDigital),
            237 => Some(TvSatellite),
            238 => Some(TvSatelliteBs),
            239 => Some(TvSatelliteCs),
            240 => Some(TvSatelliteService),
            241 => Some(TvNetwork),
            242 => Some(TvAntennaCable),
            243 => Some(TvInputHdmi1),
            244 => Some(TvInputHdmi2),
            245 => Some(TvInputHdmi3),
            246 => Some(TvInputHdmi4),
            247 => Some(TvInputComposite1),
            248 => Some(TvInputComposite2),
            249 => Some(TvInputComponent1),
            250 => Some(TvInputComponent2),
            251 => Some(TvInputVga1),
            252 => Some(TvAudioDescription),
            253 => Some(TvAudioDescriptionMixUp),
            254 => Some(TvAudioDescriptionMixDown),
            255 => Some(TvZoomMode),
            256 => Some(TvContentsMenu),
            257 => Some(TvMediaContextMenu),
            258 => Some(TvTimerProgramming),
            259 => Some(Help),
            260 => Some(NavigatePrevious),
            261 => Some(NavigateNext),
            262 => Some(NavigateIn),
            263 => Some(NavigateOut),
            264 => Some(StemPrimary),
            265 => Some(Stem1),
            266 => Some(Stem2),
            267 => Some(Stem3),
            268 => Some(DpadUpLeft),
            269 => Some(DpadDownLeft),
            270 => Some(DpadUpRight),
            271 => Some(DpadDownRight),
            272 => Some(MediaSkipForward),
            273 => Some(MediaSkipBackward),
            274 => Some(MediaStepForward),
            275 => Some(MediaStepBackward),
            276 => Some(SoftSleep),
            277 => Some(Cut),
            278 => Some(Copy),
            279 => Some(Paste),
            280 => Some(SystemNavigationUp),
            281 => Some(SystemNavigationDown),
            282 => Some(SystemNavigationLeft),
            283 => Some(SystemNavigationRight),
            284 => Some(AllApps),
            285 => Some(ThumbsUp),
            286 => Some(ThumbsDown),
            287 => Some(ProfileSwitch),
            _ => None,
        }
    }

    pub fn valid_text(&self) -> bool {
        use AndroidKeys::*;
        matches!(
            self,
            A | B
                | C
                | D
                | E
                | F
                | G
                | H
                | I
                | J
                | K
                | L
                | M
                | N
                | O
                | P
                | Q
                | R
                | S
                | T
                | U
                | V
                | W
                | X
                | Y
                | Z
                | Key0
                | Key1
                | Key2
                | Key3
                | Key4
                | Key5
                | Key6
                | Key7
                | Key8
                | Key9
                | Apostrophe
                | Backslash
                | Slash
                | Grave
                | Comma
                | Equals
                | LeftBracket
                | Plus
                | Minus
                | Period
                | RightBracket
                | Semicolon
                | Space
                | Back
                | Pound
                | Star
                | At
        )
    }

    pub fn egui_key(&self) -> Option<egui::Key> {
        use AndroidKeys::*;
        let key = match self {
            A => egui::Key::A,
            B => egui::Key::B,
            C => egui::Key::C,
            D => egui::Key::D,
            E => egui::Key::E,
            F => egui::Key::F,
            G => egui::Key::G,
            H => egui::Key::H,
            I => egui::Key::I,
            J => egui::Key::J,
            K => egui::Key::K,
            L => egui::Key::L,
            M => egui::Key::M,
            N => egui::Key::N,
            O => egui::Key::O,
            P => egui::Key::P,
            Q => egui::Key::Q,
            R => egui::Key::R,
            S => egui::Key::S,
            T => egui::Key::T,
            U => egui::Key::U,
            V => egui::Key::V,
            W => egui::Key::W,
            X => egui::Key::X,
            Y => egui::Key::Y,
            Z => egui::Key::Z,

            Key0 => egui::Key::Num0,
            Key1 => egui::Key::Num1,
            Key2 => egui::Key::Num2,
            Key3 => egui::Key::Num3,
            Key4 => egui::Key::Num4,
            Key5 => egui::Key::Num5,
            Key6 => egui::Key::Num6,
            Key7 => egui::Key::Num7,
            Key8 => egui::Key::Num8,
            Key9 => egui::Key::Num9,

            Minus => egui::Key::Minus,

            DpadLeft => egui::Key::ArrowLeft,
            DpadRight => egui::Key::ArrowRight,
            DpadUp => egui::Key::ArrowUp,
            DpadDown => egui::Key::ArrowDown,

            F1 => egui::Key::F1,
            F2 => egui::Key::F2,
            F3 => egui::Key::F3,
            F4 => egui::Key::F4,
            F5 => egui::Key::F5,
            F6 => egui::Key::F6,
            F7 => egui::Key::F7,
            F8 => egui::Key::F8,
            F9 => egui::Key::F9,
            F10 => egui::Key::F10,
            F11 => egui::Key::F11,
            F12 => egui::Key::F12,

            Escape => egui::Key::Escape,
            Tab => egui::Key::Tab,
            Enter => egui::Key::Enter,
            Space => egui::Key::Space,

            Insert => egui::Key::Insert,
            Delete => egui::Key::Backspace,
            Home => egui::Key::Home,
            MoveHome => egui::Key::Home,
            MoveEnd => egui::Key::End,
            PageUp => egui::Key::PageUp,
            PageDown => egui::Key::PageDown,

            _ => return None,
        };

        Some(key)
    }
}
