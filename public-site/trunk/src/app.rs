use lb_rs::{blocking::Lb, model::core_config::Config, Uuid};
use workspace_rs::{
    tab::{markdown_editor::Editor, svg_editor::SVGEditor},
    workspace::Workspace,
};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct LbWebApp {
    workspace: Workspace,
    editor: Option<Editor>,
    canvas: Option<SVGEditor>,
    initial_screen: InitialScreen,
}

#[derive(PartialEq)]
pub enum InitialScreen {
    Canvas,
    Editor,
}
impl LbWebApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, initial_screen: InitialScreen) -> Self {
        let ctx = cc.egui_ctx.clone();

        let lb = Lb::init(Config {
            logs: false,
            colored_logs: false,
            writeable_path: "".into(),
            background_work: false,
            stdout_logs: false,
        })
        .unwrap();

        // let martian = include_bytes!("../assets/martian.ttf");
        // let martian_bold = include_bytes!("../assets/martian-bold.ttf");
        let mut fonts = egui::FontDefinitions::default();

        workspace_rs::register_fonts(&mut fonts);
        // fonts
        //     .font_data
        //     .insert("pt_sans".to_string(), FontData::from_static(martian));
        // fonts
        //     .font_data
        //     .insert("pt_mono".to_string(), FontData::from_static(martian));
        // fonts
        //     .font_data
        //     .insert("pt_bold".to_string(), FontData::from_static(martian_bold));

        // fonts
        //     .families
        //     .insert(FontFamily::Name(Arc::from("Bold")), vec!["pt_bold".to_string()]);

        ctx.set_fonts(fonts);
        ctx.set_zoom_factor(0.9);

        ctx.set_visuals(generate_visuals());

        Self { workspace: Workspace::new(&lb, &ctx), editor: None, canvas: None, initial_screen }
    }
}

fn generate_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.extreme_bg_color = egui::Color32::from_hex("#1A1A1A").unwrap();
    visuals.code_bg_color = egui::Color32::from_hex("#282828").unwrap();
    visuals.faint_bg_color = visuals.code_bg_color;
    visuals.widgets.noninteractive.bg_fill = visuals.extreme_bg_color;

    visuals
}

impl eframe::App for LbWebApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(ctx.style().visuals.widgets.noninteractive.bg_fill))
            .show(ctx, |ui| {

                if self.editor.is_none() && self.initial_screen == InitialScreen::Editor {
                    self.editor = Some(Editor::new(
                        self.workspace.core.clone(),
                        r#"# Hello web surfer

Welcome to Lockbook! This is an example note to help you get started with our note editor. You can keep it to use as a cheat sheet or delete it anytime.

Lockbook uses Markdown, a lightweight language for formatting plain text. You can use all our supported formatting just by typing. Here’s how it works:

# This is a heading

For italic, use single *asterisks* or _underscores_.

For bold, use double **asterisks** or __underscores__.

For inline code, use single `backticks`

For code blocks, use
```
triple
backticks
```

>For block quotes,
use a greater-than sign

Bulleted list items
* start
* with
* asterisks
- or
- hyphens
+ or
+ plus
+ signs

Numbered list items
1. start
2. with
3. numbers
4. and
5. periods

Happy note taking! You can report any issues to our [Github project](https://github.com/lockbook/lockbook/issues/new) or join our [Discord server](https://discord.gg/qv9fmAZCm6)."#,
                        Uuid::new_v4(),
                        None,
                        false,
                        false,
                    ));
                }

                if self.canvas.is_none() && self.initial_screen == InitialScreen::Canvas {
                    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="M314.49859619140625 345.36351013183594 L 315.49859619140625 346.36351013183594 L 314.49859619140625 344.32054138183594 L 315.32672119140625 342.25413513183594 L 317.81109619140625 341.96507263183594 L 317.81109619140625 344.32835388183594 L 316.94781494140625 346.40647888183594 L 314.06500244140625 347.94163513183594 L 311.60797119140625 348.56663513183594 L 309.85406494140625 346.78929138183594 L 309.85406494140625 344.69944763183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='b72dc997-68e0-4dc0-9fcb-9f6eca679aa0' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M311.33843994140625 313.06663513183594 L 312.33843994140625 314.06663513183594 L 311.33843994140625 316.52757263183594 L 311.33843994140625 320.86351013183594 L 311.33843994140625 324.07054138183594 L 311.33843994140625 327.81272888183594 L 311.33843994140625 330.53929138183594 L 311.33843994140625 332.61741638183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='c0fa88a5-04c4-4b54-a584-0b463a953684' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M276.95953369140625 333.51585388183594 L 277.95953369140625 334.51585388183594 L 276.95953369140625 336.44554138183594 L 278.57281494140625 335.09397888183594 L 279.99859619140625 333.65647888183594 L 281.81890869140625 331.19944763183594 L 283.58453369140625 329.10960388183594 L 285.21343994140625 327.43772888183594 L 285.98687744140625 330.12522888183594 L 285.98687744140625 332.93772888183594 L 285.69781494140625 335.21897888183594 L 286.65875244140625 332.80491638183594 L 287.59625244140625 330.92210388183594 L 289.07672119140625 328.83226013183594 L 290.39312744140625 330.41429138183594 L 290.39312744140625 332.85960388183594 L 289.86968994140625 335.03147888183594 L 290.75250244140625 332.98069763183594 L 292.45562744140625 330.58616638183594 L 294.22125244140625 328.49632263183594 L 295.61968994140625 327.00413513183594 L 297.65093994140625 327.00413513183594 L 299.65875244140625 326.74241638183594 L 300.77203369140625 324.49632263183594 L 300.77203369140625 322.40647888183594 L 300.77203369140625 320.22679138183594 L 298.52593994140625 320.22679138183594 L 296.80328369140625 321.36741638183594 L 295.15484619140625 322.99632263183594 L 295.15484619140625 325.90257263183594 L 295.15484619140625 327.98069763183594 L 295.69781494140625 330.20726013183594 L 298.23687744140625 330.98069763183594 L 300.31500244140625 330.98069763183594 L 302.76031494140625 330.09007263183594 L 304.83843994140625 328.31272888183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='e24d5d9f-5157-438f-a208-34ea282a6c73' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M258.51812744140625 336.81272888183594 L 259.51812744140625 337.81272888183594 L 258.51812744140625 340.14866638183594 L 258.51812744140625 342.65257263183594 L 259.03375244140625 344.62132263183594 L 261.12359619140625 344.56272888183594 L 262.23687744140625 342.57835388183594 L 263.14703369140625 340.12132263183594 L 263.79937744140625 337.81272888183594 L 264.45172119140625 335.50413513183594 L 264.45172119140625 333.41429138183594 L 264.45172119140625 335.62913513183594 L 264.45172119140625 338.35569763183594 L 264.45172119140625 341.08226013183594 L 264.45172119140625 344.28929138183594 L 264.45172119140625 347.01585388183594 L 264.45172119140625 349.74241638183594 L 264.76422119140625 352.04319763183594 L 264.76422119140625 354.85569763183594 L 264.47515869140625 357.30101013183594 L 263.13140869140625 359.26194763183594 L 260.67437744140625 361.08226013183594 L 258.69000244140625 361.93772888183594 L 257.56500244140625 359.99632263183594 L 257.24859619140625 357.68772888183594 L 257.24859619140625 355.37913513183594 L 257.24859619140625 352.92210388183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='134826da-3ae6-4027-97ff-600ef0f5c03a' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M244.70172119140625 341.24241638183594 L 245.70172119140625 342.24241638183594 L 244.70172119140625 344.10569763183594 L 244.70172119140625 346.12913513183594 L 244.70172119140625 348.14476013183594 L 244.70172119140625 350.26194763183594 L 245.84234619140625 348.34397888183594 L 246.52203369140625 345.60960388183594 L 247.54156494140625 342.87522888183594 L 248.47906494140625 340.99241638183594 L 249.93218994140625 339.21507263183594 L 252.04937744140625 338.66429138183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='5a0917fa-d2d2-4c36-b6ea-2e4dde6b61d2' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M224.88140869140625 327.11741638183594 L 225.88140869140625 328.11741638183594 L 227.68609619140625 327.11741638183594 L 230.89312744140625 327.11741638183594 L 235.11578369140625 326.34788513183594 L 238.32281494140625 325.60569763183594 L 242.54547119140625 324.43772888183594 L 245.32672119140625 323.75022888183594 L 247.77203369140625 323.14476013183594 L 249.99859619140625 322.88304138183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='8ccfbc1f-5403-4d6f-b68c-615ac439b793' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M238.47515869140625 326.66819763183594 L 239.47515869140625 327.66819763183594 L 238.47515869140625 330.58616638183594 L 238.47515869140625 333.55882263183594 L 238.47515869140625 336.53147888183594 L 238.47515869140625 340.85569763183594 L 238.47515869140625 345.19163513183594 L 238.47515869140625 347.57054138183594 L 238.47515869140625 349.94944763183594 L 238.47515869140625 352.67601013183594 L 238.47515869140625 355.17991638183594 L 238.47515869140625 357.20335388183594" stroke-width='1' stroke='rgba(128,0,128,1)' fill='none' id='1b27bf09-72ed-4025-847a-99176f1e986d' transform='matrix(1 0 0 1 75.29297 -274.03125)'/><path d="M201.46734619140625 515.8166351318359 L 202.46734619140625 516.8166351318359 L 200.99468994140625 515.2736663818359 L 199.73687744140625 517.7775726318359 L 198.70953369140625 520.5041351318359 L 198.02203369140625 523.2306976318359 L 197.70562744140625 525.5314788818359 L 198.84625244140625 527.4103851318359 L 202.08453369140625 527.4103851318359 L 204.81109619140625 526.0431976318359 L 206.91265869140625 523.6213226318359 L 208.69781494140625 520.4064788818359 L 209.71734619140625 517.6721038818359 L 210.34234619140625 515.2150726318359 L 208.93218994140625 513.5158538818359 L 206.37359619140625 513.5158538818359 L 207.22515869140625 515.7150726318359 L 210.00640869140625 515.7150726318359 L 212.30718994140625 515.7150726318359 L 214.60797119140625 515.7150726318359 L 216.77984619140625 515.7150726318359 L 217.59234619140625 518.0900726318359 L 217.59234619140625 520.3908538818359 L 217.59234619140625 522.6916351318359 L 217.59234619140625 525.1369476318359 L 220.18609619140625 525.6525726318359 L 221.77984619140625 524.0510101318359 L 223.51031494140625 521.6291351318359 L 224.76031494140625 519.7463226318359 L 225.67047119140625 517.2892913818359 L 225.67047119140625 515.2502288818359 L 223.31890869140625 515.2502288818359 L 226.10797119140625 513.3244476318359 L 230.33062744140625 512.1564788818359 L 233.30328369140625 511.73069763183594 L 236.27593994140625 510.87913513183594" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='bcb9da1b-1231-4749-a709-fbe945218ab1' transform='matrix(1 0 0 1 0 0)'/><path d="M170.90875244140625 512.8830413818359 L 171.90875244140625 513.8830413818359 L 175.15875244140625 512.8830413818359 L 178.36578369140625 512.8830413818359 L 182.58843994140625 512.8830413818359 L 184.88922119140625 512.8830413818359 L 187.39312744140625 512.8830413818359" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='80d9f6ce-8685-4e88-9f6a-adca6e34ee52' transform='matrix(1 0 0 1 0 0)'/><path d="M187.29937744140625 487.26194763183594 L 188.29937744140625 488.26194763183594 L 186.92828369140625 490.69944763183594 L 185.65093994140625 493.67210388183594 L 184.37359619140625 496.64476013183594 L 182.44781494140625 500.96897888183594 L 180.52203369140625 505.29319763183594 L 178.48687744140625 510.37522888183594 L 176.45172119140625 515.4572601318359 L 174.52593994140625 519.7814788818359 L 172.90484619140625 524.1174163818359 L 171.99859619140625 527.7385101318359 L 170.83062744140625 531.9611663818359 L 170.83062744140625 534.2619476318359 L 173.55718994140625 535.6877288818359 L 178.37359619140625 535.6877288818359 L 183.19000244140625 534.8361663818359 L 185.56890869140625 533.6447601318359" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='853248cc-01c1-4ea3-a5cc-e2cf9ff2db85' transform='matrix(1 0 0 1 0 0)'/><path d="M118.90875244140625 481.92210388183594 L 119.90875244140625 482.92210388183594 L 117.30718994140625 481.92210388183594 L 113.51812744140625 481.94554138183594 L 111.09625244140625 484.04710388183594 L 109.04156494140625 486.09397888183594 L 107.30328369140625 488.50804138183594 L 106.30328369140625 490.80882263183594 L 106.30328369140625 493.25413513183594 L 108.41656494140625 494.63304138183594 L 111.19781494140625 494.63304138183594 L 113.69781494140625 492.12522888183594 L 115.90875244140625 489.16429138183594 L 117.01422119140625 487.31663513183594 L 118.74468994140625 484.89476013183594 L 119.62750244140625 482.80491638183594 L 119.08843994140625 486.85179138183594 L 118.71734619140625 489.63304138183594 L 118.34625244140625 492.84007263183594 L 118.34625244140625 495.14085388183594 L 120.53765869140625 495.99632263183594 L 122.91656494140625 495.99632263183594 L 124.39312744140625 494.51585388183594 L 126.97515869140625 491.55491638183594 L 129.85797119140625 488.18772888183594 L 131.98687744140625 485.31272888183594 L 133.09234619140625 483.46507263183594 L 134.82281494140625 481.04319763183594 L 135.96343994140625 479.26585388183594 L 135.16265869140625 482.01976013183594 L 134.44781494140625 485.22679138183594 L 133.65093994140625 487.60569763183594 L 132.93609619140625 490.81272888183594 L 132.56500244140625 494.01976013183594 L 132.56500244140625 496.32054138183594 L 135.79156494140625 495.35179138183594 L 136.89703369140625 493.50413513183594 L 139.10797119140625 490.17601013183594 L 140.81109619140625 487.78147888183594 L 141.97906494140625 485.69163513183594 L 142.75250244140625 488.48460388183594 L 142.75250244140625 490.78538513183594 L 142.75250244140625 493.08616638183594 L 142.75250244140625 495.16429138183594 L 144.93609619140625 495.16429138183594 L 147.49078369140625 493.57444763183594 L 148.59625244140625 491.72679138183594 L 150.18218994140625 489.73851013183594 L 152.04937744140625 485.98851013183594 L 153.63531494140625 484.00022888183594 L 155.42047119140625 481.15257263183594 L 157.18609619140625 479.06272888183594 L 159.32281494140625 480.89476013183594 L 163.54547119140625 480.89476013183594 L 165.92437744140625 480.49632263183594 L 168.89703369140625 479.21897888183594 L 172.06500244140625 477.40647888183594 L 175.23297119140625 475.59397888183594 L 178.40093994140625 473.78147888183594 L 184.33843994140625 470.55491638183594 L 187.95953369140625 468.74241638183594 L 190.93218994140625 467.46507263183594" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='bf4dc74a-a471-438f-ac87-af6fb9557d64' transform='matrix(1 0 0 1 0 0)'/><path d="M50.43609619140625 497.41819763183594 L 51.43609619140625 498.41819763183594 L 48.62359619140625 497.41819763183594 L 46.27203369140625 497.41819763183594 L 44.03765869140625 498.28147888183594 L 43.37750244140625 500.58226013183594 L 42.26812744140625 502.42601013183594 L 42.26812744140625 504.72679138183594 L 41.95172119140625 507.02757263183594 L 41.95172119140625 509.32835388183594 L 43.14703369140625 511.22679138183594 L 46.97515869140625 509.26585388183594 L 51.26422119140625 504.97288513183594 L 54.82281494140625 500.90257263183594 L 60.40484619140625 494.07444763183594 L 65.36578369140625 486.62913513183594 L 72.12750244140625 477.83226013183594 L 77.53765869140625 468.35960388183594 L 82.72125244140625 459.92991638183594 L 84.64312744140625 455.60179138183594 L 88.36187744140625 447.53538513183594 L 90.72906494140625 439.83226013183594 L 91.52203369140625 437.44944763183594 L 92.48297119140625 433.12132263183594 L 93.33062744140625 430.14476013183594 L 93.67047119140625 427.41038513183594 L 91.31890869140625 431.65257263183594 L 88.77593994140625 436.22679138183594 L 86.85015869140625 440.55101013183594 L 84.16656494140625 445.91429138183594 L 81.34234619140625 452.12132263183594 L 79.19390869140625 458.01976013183594 L 77.15875244140625 463.10179138183594 L 75.01031494140625 469.00022888183594 L 71.52984619140625 478.40647888183594 L 70.08453369140625 482.73069763183594 L 69.06500244140625 487.81272888183594 L 68.10015869140625 492.13694763183594 L 67.13531494140625 496.46116638183594 L 66.70953369140625 501.27757263183594 L 66.70953369140625 503.65647888183594 L 66.70953369140625 505.95726013183594 L 69.84234619140625 505.30491638183594 L 71.96343994140625 503.17991638183594 L 75.32672119140625 499.81272888183594 L 78.20953369140625 496.44554138183594 L 79.68609619140625 494.96507263183594 L 81.80718994140625 492.41429138183594 L 83.92828369140625 490.28929138183594 L 86.04937744140625 488.16429138183594 L 88.17828369140625 485.65647888183594 L 89.63140869140625 484.16429138183594 L 90.14703369140625 486.25022888183594 L 90.14703369140625 488.62913513183594 L 90.14703369140625 492.37132263183594 L 90.14703369140625 494.75022888183594 L 90.14703369140625 498.43772888183594 L 90.14703369140625 500.73851013183594 L 93.73687744140625 499.91819763183594 L 95.72125244140625 498.32835388183594 L 97.84234619140625 496.20335388183594 L 99.31890869140625 494.72288513183594 L 101.16265869140625 493.61351013183594 L 102.63922119140625 492.13304138183594" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='51138615-5aa3-4524-b6f9-bf3ef731e6cc' transform='matrix(1 0 0 1 0 0)'/><path d="M181.94781494140625 390.29710388183594 L 182.94781494140625 391.29710388183594 L 180.53375244140625 390.29710388183594 L 177.17047119140625 390.29710388183594 L 174.32281494140625 391.40257263183594 L 172.84234619140625 392.87913513183594 L 171.73297119140625 394.72288513183594 L 170.54156494140625 397.10179138183594 L 169.74468994140625 399.48069763183594 L 168.65875244140625 402.68772888183594 L 167.86187744140625 405.06663513183594 L 167.86187744140625 408.75413513183594 L 167.86187744140625 411.53538513183594 L 167.86187744140625 413.61351013183594 L 170.37359619140625 413.85569763183594 L 173.51812744140625 410.70335388183594 L 176.06500244140625 408.57835388183594 L 178.18609619140625 406.02757263183594 L 180.44781494140625 402.85569763183594 L 182.25640869140625 399.68382263183594 L 184.12359619140625 395.93382263183594 L 185.60015869140625 392.23851013183594 L 186.48297119140625 390.14866638183594 L 186.48297119140625 393.19944763183594 L 186.48297119140625 396.17210388183594 L 186.08453369140625 398.55101013183594 L 185.74078369140625 402.29319763183594 L 185.74078369140625 404.67210388183594 L 185.74078369140625 408.35960388183594 L 186.02593994140625 410.86351013183594 L 187.95953369140625 411.48851013183594 L 190.26031494140625 411.48851013183594 L 192.24468994140625 409.89866638183594 L 193.83062744140625 407.91038513183594 L 195.95172119140625 405.35960388183594 L 198.07281494140625 403.23460388183594 L 199.77203369140625 400.68382263183594 L 202.46343994140625 397.21507263183594 L 204.04937744140625 395.22679138183594 L 206.17828369140625 392.71897888183594 L 207.51422119140625 390.75022888183594 L 208.60015869140625 393.33226013183594 L 207.54547119140625 396.53929138183594 L 206.80328369140625 400.22679138183594 L 206.43218994140625 403.43382263183594 L 206.43218994140625 405.51194763183594 L 209.21343994140625 405.76976013183594 L 212.56109619140625 404.17991638183594 L 214.68218994140625 402.05491638183594 L 216.80328369140625 399.50413513183594 L 219.06500244140625 396.33226013183594 L 221.46734619140625 392.48460388183594 L 224.27203369140625 388.45335388183594 L 225.37750244140625 386.60569763183594 L 226.85406494140625 385.12522888183594 L 228.30718994140625 383.60960388183594 L 228.56500244140625 385.96897888183594 L 228.56500244140625 388.34788513183594 L 228.56500244140625 391.32054138183594 L 228.56500244140625 394.52757263183594 L 228.56500244140625 398.26976013183594 L 230.69390869140625 399.20726013183594 L 233.07281494140625 399.20726013183594 L 236.04547119140625 399.20726013183594 L 239.01812744140625 399.20726013183594 L 242.63922119140625 397.39476013183594 L 249.15484619140625 393.24632263183594 L 251.27593994140625 391.12132263183594 L 258.40484619140625 383.98851013183594 L 260.52593994140625 381.43772888183594" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='8db087e6-5913-4710-8762-dc259e77652e' transform='matrix(1 0 0 1 0 0)'/><path d="M162.20953369140625 376.36351013183594 L 163.20953369140625 377.36351013183594 L 160.80328369140625 376.67601013183594 L 158.95562744140625 377.78147888183594 L 157.47515869140625 379.25804138183594 L 155.05328369140625 380.98851013183594 L 150.65875244140625 384.89476013183594 L 147.36578369140625 389.08616638183594 L 146.17437744140625 391.46507263183594 L 144.74859619140625 394.67210388183594 L 143.95172119140625 397.05101013183594 L 143.49859619140625 400.67210388183594 L 143.49859619140625 406.35960388183594 L 143.49859619140625 408.73851013183594 L 143.49859619140625 411.94554138183594 L 144.60406494140625 413.78929138183594 L 146.19000244140625 415.77366638183594 L 148.77203369140625 418.35569763183594 L 151.15875244140625 420.05882263183594 L 154.36578369140625 421.47679138183594" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='793a0a29-842e-4635-b23f-a1d3f269bb58' transform='matrix(1 0 0 1 0 0)'/><path d="M-4.10296630859375 349.30101013183594 L -3.10296630859375 350.30101013183594 L -4.10296630859375 352.25804138183594 L -4.10296630859375 355.46507263183594 L -4.10296630859375 358.19163513183594 L -4.10296630859375 360.57054138183594 L -4.10296630859375 363.77757263183594 L -4.10296630859375 368.00022888183594 L -4.10296630859375 370.30101013183594 L -2.64984130859375 371.80882263183594 L 0.55718994140625 371.80882263183594 L 4.77984619140625 370.61741638183594 L 7.49468994140625 367.89866638183594 L 10.04156494140625 365.77366638183594 L 12.92437744140625 362.40647888183594 L 16.15093994140625 358.37522888183594 L 19.20172119140625 353.79710388183594 L 21.06890869140625 350.04710388183594 L 22.20172119140625 345.81663513183594 L 22.90875244140625 342.60179138183594 L 22.90875244140625 340.29319763183594 L 20.16265869140625 343.83616638183594 L 19.25640869140625 347.45726013183594 L 17.81109619140625 351.78147888183594 L 16.36578369140625 356.10569763183594 L 15.29156494140625 362.00413513183594 L 12.58453369140625 372.82444763183594 L 11.39703369140625 380.52366638183594 L 9.61968994140625 388.22288513183594 L 8.32281494140625 397.94554138183594 L 6.37750244140625 407.66819763183594 L 4.34625244140625 418.48851013183594 L 3.04937744140625 428.21116638183594 L 1.69390869140625 439.03147888183594 L 1.18218994140625 444.11351013183594 L -0.05999755859375 452.79710388183594 L -1.30218505859375 461.48069763183594 L -2.15374755859375 464.45335388183594 L -3.68109130859375 469.53538513183594 L -5.04046630859375 473.15647888183594 L -6.31781005859375 476.12913513183594 L -8.90765380859375 478.71116638183594 L -11.21624755859375 479.36351013183594 L -13.52484130859375 478.73460388183594 L -14.69281005859375 473.91038513183594 L -15.14593505859375 470.28538513183594 L -15.14593505859375 465.95726013183594 L -15.14593505859375 461.62913513183594 L -15.14593505859375 458.00413513183594 L -14.18499755859375 453.67601013183594 L -12.26312255859375 449.34788513183594 L -9.72406005859375 444.76976013183594 L -7.18499755859375 440.19163513183594 L -1.35296630859375 431.76194763183594 L 0.90875244140625 428.59007263183594 L 5.42047119140625 423.50804138183594 L 9.93218994140625 417.86351013183594 L 15.01031494140625 412.78147888183594 L 19.29937744140625 408.48851013183594 L 23.81109619140625 403.40647888183594 L 28.32281494140625 398.32444763183594 L 31.88140869140625 394.25413513183594 L 34.93218994140625 390.18382263183594 L 37.19390869140625 387.01194763183594 L 38.29937744140625 385.16429138183594 L 39.99859619140625 382.61351013183594 L 41.10406494140625 380.76585388183594 L 42.15484619140625 377.97679138183594 L 41.86968994140625 381.99241638183594 L 40.67828369140625 384.37132263183594 L 39.82672119140625 387.34397888183594 L 38.29156494140625 391.67991638183594 L 37.49468994140625 394.05882263183594 L 36.38140869140625 397.74632263183594 L 36.38140869140625 400.47288513183594 L 36.38140869140625 402.49632263183594 L 38.82672119140625 402.49632263183594 L 40.92828369140625 400.44554138183594 L 43.04937744140625 398.32054138183594 L 45.17828369140625 395.44554138183594 L 46.98687744140625 392.27366638183594 L 48.57281494140625 388.35569763183594 L 49.27984619140625 385.14085388183594 L 49.64703369140625 381.44554138183594 L 49.64703369140625 378.71116638183594 L 48.13140869140625 376.90647888183594 L 45.67437744140625 376.90647888183594 L 44.41656494140625 378.84007263183594 L 43.38922119140625 381.56663513183594 L 43.38922119140625 384.29319763183594 L 43.38922119140625 386.73851013183594 L 45.80328369140625 387.64866638183594 L 50.02593994140625 387.64866638183594 L 52.40484619140625 387.25022888183594 L 54.78375244140625 386.05882263183594 L 57.33062744140625 384.35569763183594 L 59.87750244140625 382.23069763183594 L 62.94000244140625 379.13304138183594 L 64.41656494140625 377.65257263183594 L 66.46343994140625 375.59788513183594 L 66.49468994140625 377.69163513183594 L 64.56109619140625 381.91429138183594 L 62.62750244140625 386.13694763183594 L 61.48687744140625 390.35960388183594 L 60.37359619140625 394.04710388183594 L 60.37359619140625 396.34788513183594 L 60.37359619140625 398.42601013183594 L 62.67437744140625 398.42601013183594 L 65.05328369140625 398.42601013183594 L 66.89703369140625 397.31663513183594 L 69.01812744140625 394.76585388183594 L 71.73297119140625 392.04710388183594 L 74.44781494140625 389.32835388183594 L 78.60406494140625 383.26976013183594 L 80.38922119140625 380.05491638183594 L 81.57672119140625 377.67210388183594 L 83.02203369140625 374.82444763183594 L 83.95953369140625 372.94163513183594 L 83.95953369140625 376.40647888183594 L 83.10797119140625 379.37913513183594 L 82.20172119140625 383.00022888183594 L 81.74859619140625 386.62132263183594 L 81.29547119140625 390.24241638183594 L 80.49859619140625 395.05882263183594 L 80.49859619140625 398.03147888183594 L 80.49859619140625 400.41038513183594 L 80.49859619140625 402.78929138183594 L 80.49859619140625 405.16819763183594 L 83.36578369140625 406.55882263183594 L 86.33843994140625 406.98069763183594 L 89.31109619140625 406.98069763183594 L 92.28375244140625 406.55491638183594 L 95.45172119140625 404.74241638183594 L 98.81500244140625 401.85569763183594" stroke-width='5' stroke='rgba(0,0,0,1)' fill='none' id='3a59007b-5ecb-4118-9b2d-65722148c1eb' transform='matrix(1 0 0 1 0 0)'/><g id="lb_master_transform" transform="matrix(1 0 0 1 602.1928 -12.461166)"></g> </svg>
                    "#;
                    self.canvas  = Some(
                        SVGEditor::new(svg.as_bytes(), ui.ctx(), self.workspace.core.clone(), Uuid::new_v4(), None, None)
                    )
                }
                if let Some(md) = &mut self.editor {
                    egui::Frame::default().show(ui, |ui|{
                        md.show(ui);
                    });
                    // ui.centered_and_justified(|ui| {
                    //     ui.vertical(|ui| {
                    //         ui.centered_and_justified(|ui| {
                    //         });
                    //     });
                    // });
                }

                if let Some(svg) = &mut self.canvas {
                    egui::Frame::default().show(ui, |ui|{
                        svg.show(ui);
                    });
                    // ui.centered_and_justified(|ui| {
                    //     ui.vertical(|ui| {
                    //         ui.centered_and_justified(|ui| {
                    //         });
                    //     });
                    // });
                }
            });
    }
}
