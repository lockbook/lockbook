//! Headless observation harness — the AI/agent-facing projection of the action
//! surface. Each scenario drives a scripted sequence of tree `Action`s and then
//! captures the resulting state: a structured `readout` (pure, no GPU) and/or a
//! PNG rendered via egui_kittest's wgpu backend. A state reached by a script is
//! the same one a click reaches, so this both *sees* and *asserts* the app
//! without a window.
//!
//! `filmstrip` samples a range of scroll offsets and stitches the sidebar crops
//! side by side — the way to observe a scroll transition (e.g. sticky-header
//! push-off) in a single image, since no settled frame captures an animation.
//!
//! GPU-bound, so `#[ignore]` by default. Run on demand:
//!   cargo test -p lockbook-egui --test observe -- --ignored --nocapture
//! Output lands in `target/tmp/observe/`; readouts print to stdout.

use std::path::PathBuf;

use egui_kittest::Harness;
use image::{Rgba, RgbaImage, imageops};
use lockbook_egui::file_tree::{Action, FileTree, demo_files, demo_micro};
use lockbook_egui::{Lockbook, Uuid};

const WIN: [f32; 2] = [1300.0, 800.0];
const SIDEBAR_W: f32 = 260.0;
const CROP_H: f32 = 460.0;

/// Demo-tree ids mirror `demo_files`' deterministic `from_u128` layout:
/// 1=Apps, 2=lockbook, 3=clients, 4=egui, 5=src, 12=Documents, 100=button.rs.
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn out_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("observe");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Render one settled frame for a scripted state at an optional forced scroll.
fn render_frame(script: &[Action], scroll: Option<f32>) -> RgbaImage {
    let script = script.to_vec();
    let mut app: Option<Lockbook> = None;
    let mut applied = false;

    let mut harness = Harness::builder().with_size(WIN).wgpu().build(|ctx| {
        // Frame 1: install fonts/theme, bail before drawing — `set_fonts` lands
        // next frame and the design system needs the "Bold" family.
        if app.is_none() {
            let mut a = Lockbook::new(ctx);
            a.deferred_init(ctx);
            app = Some(a);
            ctx.request_repaint();
            return;
        }
        let app = app.as_mut().unwrap();
        if !applied {
            for act in &script {
                app.drive(act.clone());
            }
            applied = true;
        }
        // Re-assert the offset every frame so the captured frame is pinned there
        // regardless of how many settle-frames `run` takes.
        if let Some(y) = scroll {
            app.drive(Action::ScrollTo(y));
        }
        app.update(ctx);
    });
    harness.run();
    harness.render().expect("wgpu render failed")
}

/// The sidebar region of a frame, cropped from its top-left.
fn sidebar_crop(img: &RgbaImage) -> RgbaImage {
    let scale = img.width() as f32 / WIN[0];
    let w = (SIDEBAR_W * scale) as u32;
    let h = ((CROP_H * scale) as u32).min(img.height());
    imageops::crop_imm(img, 0, 0, w, h).to_image()
}

fn observe(name: &str, script: Vec<Action>) {
    // Projection 1 — the pure readout, no GPU.
    let files = demo_files();
    let mut tree = FileTree::default();
    for a in &script {
        let _ = tree.apply(a.clone(), &files);
    }
    let readout = tree.readout(&files);

    // Projection 2 — the screencap.
    let path = out_dir().join(format!("{name}.png"));
    render_frame(&script, None).save(&path).unwrap();

    println!("\n=== observe: {name} ===");
    println!("png: {}", path.display());
    println!("readout:\n{readout}");
}

/// Sample the sidebar across a set of scroll offsets and stitch the crops into a
/// single left-to-right filmstrip.
fn filmstrip(name: &str, script: Vec<Action>, offsets: &[f32]) {
    let gap = 3u32;
    let bg = Rgba([48, 48, 48, 255]);
    let frames: Vec<RgbaImage> = offsets
        .iter()
        .map(|&off| sidebar_crop(&render_frame(&script, Some(off))))
        .collect();

    let (fw, fh) = (frames[0].width(), frames[0].height());
    let n = frames.len() as u32;
    let mut strip = RgbaImage::from_pixel(fw * n + gap * (n - 1), fh, bg);
    for (i, frame) in frames.iter().enumerate() {
        imageops::overlay(&mut strip, frame, i as i64 * (fw + gap) as i64, 0);
    }

    let path = out_dir().join(format!("{name}.png"));
    strip.save(&path).unwrap();
    println!("\n=== filmstrip: {name} (offsets {offsets:?}) ===");
    println!("png: {}", path.display());
}

/// Non-visual animation observability: sweep the scroll offset in fine steps and
/// assert every row's effective on-screen position (in-flow *or* stuck) moves
/// continuously — a sticky header that teleports (the "sudden appear") shows up
/// as a `vy` jump far larger than the offset step. Pure (no GPU), runs by default.
#[test]
fn sticky_continuity() {
    use std::collections::BTreeMap;

    let files = demo_files();
    let mut tree = FileTree::default();
    for n in [1u128, 2, 3, 4, 5, 12] {
        tree.apply(Action::Toggle(id(n)), &files);
    }

    let (step, viewport) = (2.0f32, 700.0);
    let tol = step + 0.6; // a smoothly-scrolling row moves ~`step` px per step
    let mut prev: BTreeMap<usize, f32> = BTreeMap::new();
    let mut pops = Vec::new();

    for i in 0..300 {
        let off = i as f32 * step;
        let cur: BTreeMap<usize, f32> = tree
            .row_positions(&files, off, viewport)
            .into_iter()
            .collect();
        for (idx, &vy) in &cur {
            if let Some(&pvy) = prev.get(idx) {
                if (vy - pvy).abs() > tol {
                    pops.push(format!(
                        "off {off:>5.0}: row {idx:>2} vy {pvy:>6.1} -> {vy:<6.1} (Δ{:+.1})",
                        vy - pvy
                    ));
                }
            }
        }
        prev = cur;
    }

    if pops.is_empty() {
        println!("sticky handoff continuous across sweep");
    } else {
        println!("{} discontinuities:\n{}", pops.len(), pops.join("\n"));
    }
    assert!(pops.is_empty(), "sticky handoff has {} pop(s); see stdout", pops.len());
}

#[test]
#[ignore = "GPU-bound; run explicitly with --ignored"]
fn tree_states() {
    observe("collapsed", vec![]);
    observe("documents_expanded", vec![Action::Toggle(id(12))]);
    observe(
        "deep_chain",
        vec![
            Action::Toggle(id(1)), // Apps
            Action::Toggle(id(2)), // lockbook
            Action::Toggle(id(3)), // clients
            Action::Toggle(id(4)), // egui
            Action::Toggle(id(5)), // src
            Action::Select(id(100)),
        ],
    );
}

#[test]
#[ignore = "GPU-bound; run explicitly with --ignored"]
fn sticky_filmstrip() {
    // Deep chain (Apps ▸ lockbook ▸ clients ▸ egui ▸ src) + Documents for scroll
    // room. Scrolling into src's leaves grows a 5-level stuck stack; scrolling
    // past them, foundry.md (shallower) pushes the deepest header out.
    let script = vec![
        Action::Toggle(id(1)),
        Action::Toggle(id(2)),
        Action::Toggle(id(3)),
        Action::Toggle(id(4)),
        Action::Toggle(id(5)),
        Action::Toggle(id(12)),
    ];
    filmstrip("sticky", script, &[52.0, 130.0, 182.0, 234.0, 286.0, 338.0]);
}

/// Sub-pixel filmstrip of the micro tree (`aaa` rolls off, `folder` rises and
/// sticks) at fractional scroll offsets across the handoff — the case where the
/// in-flow↔stuck anchor mismatch showed up as a jump. Cropped to the top rows.
#[test]
#[ignore = "GPU-bound"]
fn micro_filmstrip() {
    let offsets: Vec<f32> = (0..=8).map(|o| 24.0 + o as f32 * 0.5).collect();
    let render = |off: f32| -> RgbaImage {
        let mut app: Option<Lockbook> = None;
        let mut init = false;
        let mut harness = Harness::builder()
            .with_size([500.0, 160.0])
            .wgpu()
            .build(|ctx| {
                if app.is_none() {
                    let mut a = Lockbook::new(ctx);
                    a.deferred_init(ctx);
                    app = Some(a);
                    ctx.request_repaint();
                    return;
                }
                let app = app.as_mut().unwrap();
                if !init {
                    app.set_files(demo_micro());
                    app.drive(Action::Toggle(id(2)));
                    init = true;
                }
                app.drive(Action::ScrollTo(off));
                app.update(ctx);
            });
        harness.run();
        let img = harness.render().expect("render");
        let scale = img.width() as f32 / 500.0;
        imageops::crop_imm(&img, 0, 0, (SIDEBAR_W * scale) as u32, (90.0 * scale) as u32).to_image()
    };
    let frames: Vec<RgbaImage> = offsets.iter().map(|&off| render(off)).collect();

    let gap = 3u32;
    let (fw, fh) = (frames[0].width(), frames[0].height());
    let n = frames.len() as u32;
    let mut strip = RgbaImage::from_pixel(fw * n + gap * (n - 1), fh, Rgba([48, 48, 48, 255]));
    for (i, f) in frames.iter().enumerate() {
        imageops::overlay(&mut strip, f, i as i64 * (fw + gap) as i64, 0);
    }
    let path = out_dir().join("micro.png");
    strip.save(&path).unwrap();
    println!("offsets {offsets:?}\npng: {}", path.display());
}

/// Numeric stack readout — the non-visual companion to the filmstrip. Prints the
/// stuck header stack (`name@vy(height)`) at the same offsets, so the stack's
/// composition and the push-off can be read exactly without a GPU. Pure; run with
/// `cargo test sticky_dump -- --nocapture`.
#[test]
fn sticky_dump() {
    let files = demo_files();
    let mut tree = FileTree::default();
    for n in [1u128, 2, 3, 4, 5, 12] {
        tree.apply(Action::Toggle(id(n)), &files);
    }
    for off in [52.0, 130.0, 182.0, 234.0, 286.0, 338.0] {
        let stack: Vec<String> = tree
            .sticky_debug(&files, off)
            .into_iter()
            .map(|(n, vy, h)| format!("{n}@{vy:.0}(h{h:.0})"))
            .collect();
        println!("off {off:>5.0}: [{}]", stack.join(", "));
    }
}
