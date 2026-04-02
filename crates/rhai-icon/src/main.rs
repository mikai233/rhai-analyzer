use clap::Parser;
use skia_safe::{
    Canvas, Color, EncodedImageFormat, Matrix, Paint, PaintStyle, Path, PathFillType, Point, Rect,
    TileMode, gradient_shader, surfaces, svg,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path as FsPath, PathBuf};

#[derive(Debug, Parser)]
#[command(author, version, about = "Generate Rhai analyzer extension icons")]
struct Args {
    #[arg(long, default_value = "icon.png")]
    output: PathBuf,

    #[arg(long)]
    format: Option<String>,

    #[arg(long, default_value_t = 1024)]
    size: i32,
}

fn main() {
    let args = Args::parse();
    let format = resolve_output_format(args.format, &args.output);
    generate_icon(args.size, format, &args.output);
}

fn resolve_output_format(format: Option<String>, output: &FsPath) -> String {
    match format {
        Some(format) => match format.to_ascii_lowercase().as_str() {
            "png" => "png".to_owned(),
            "svg" => "svg".to_owned(),
            other => panic!("Unsupported format: {other}. Expected png or svg."),
        },
        None => match output.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("svg") => "svg".to_owned(),
            _ => "png".to_owned(),
        },
    }
}

fn generate_icon(size: i32, format: String, output: &FsPath) {
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    match format.as_str() {
        "png" => write_png(size, output),
        "svg" => write_svg(size, output),
        other => panic!("Unsupported format: {other}. Expected png or svg."),
    }

    println!("Generated {} icon at {}", format, output.display());
}

fn write_png(size: i32, output: &FsPath) {
    let mut surface = surfaces::raster_n32_premul((size, size)).expect("Failed to create surface");
    let canvas = surface.canvas();
    canvas.clear(Color::TRANSPARENT);

    draw_icon(canvas, size);

    let image = surface.image_snapshot();
    let data = image
        .encode(None, EncodedImageFormat::PNG, 100)
        .expect("Failed to encode PNG");

    let mut file = File::create(output).expect("Failed to create output file");
    file.write_all(data.as_bytes())
        .expect("Failed to write output file");
}

fn write_svg(size: i32, output: &FsPath) {
    let canvas = svg::Canvas::new(Rect::from_size((size, size)), None);
    draw_icon(&canvas, size);

    let data = canvas.end();
    let mut file = File::create(output).expect("Failed to create output file");
    file.write_all(data.as_bytes())
        .expect("Failed to write output file");
}

fn draw_icon(canvas: &Canvas, size: i32) {
    draw_rhai_logo(canvas, size, 0.75);
    draw_overlay_badge(canvas, size);
}

fn draw_rhai_logo(canvas: &Canvas, size: i32, render_scale: f32) {
    let path_data = "M244.724,402.002L226.114,402.002C205.572,402.002 188.894,385.324 188.894,364.782L188.894,253.122L188.895,253.242C188.959,263.418 197.208,271.667 207.384,271.732L207.504,271.732L218.288,271.732C211.035,256.892 213.572,238.455 225.902,226.126L254.029,197.998C254.029,197.998 282.157,226.126 282.157,226.126C294.486,238.455 297.023,256.892 289.77,271.732L300.554,271.732L300.674,271.732C310.85,271.667 319.099,263.418 319.164,253.242L319.164,253.122L319.164,364.782C319.164,385.324 302.486,402.002 281.944,402.002L263.334,402.002L263.334,420.612L244.724,420.612L244.724,402.002ZM244.724,383.392L244.724,364.782C244.724,354.511 236.385,346.172 226.114,346.172L207.504,346.172L207.504,364.782C207.504,375.053 215.843,383.392 226.114,383.392L244.724,383.392ZM263.334,364.782C263.334,354.511 271.673,346.172 281.944,346.172L300.554,346.172L300.554,364.782C300.554,375.053 292.215,383.392 281.944,383.392L263.334,383.392L263.334,364.782ZM263.334,308.952C263.334,298.681 271.673,290.342 281.944,290.342L300.554,290.342L300.554,308.952C300.554,319.223 292.215,327.562 281.944,327.562L263.334,327.562L263.334,308.952ZM244.724,308.952C244.724,298.681 236.385,290.342 226.114,290.342L207.504,290.342L207.504,308.952C207.504,319.223 215.843,327.562 226.114,327.562L244.724,327.562L244.724,308.952ZM267.188,267.412C274.451,260.15 274.451,248.357 267.188,241.094C267.188,241.094 254.029,227.935 254.029,227.935L240.87,241.094C233.607,248.357 233.607,260.15 240.87,267.412L254.029,280.572L267.188,267.412Z";
    let mut path = Path::from_svg(path_data).expect("Failed to parse Rhai logo SVG path");
    path.set_fill_type(PathFillType::EvenOdd);

    let bounds = path.compute_tight_bounds();
    let target_size = size as f32 * render_scale;
    let scale = target_size / bounds.width().max(bounds.height());

    let mut matrix = Matrix::new_identity();
    matrix.pre_translate((
        size as f32 * 0.45 - (bounds.left + bounds.width() / 2.0) * scale,
        size as f32 * 0.45 - (bounds.top + bounds.height() / 2.0) * scale,
    ));
    matrix.pre_scale((scale, scale), None);
    path.transform(&matrix);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let colors = [Color::from_rgb(255, 215, 118), Color::from_rgb(246, 117, 0)];
    let transformed_bounds = path.compute_tight_bounds();
    let shader = gradient_shader::linear(
        (
            Point::new(transformed_bounds.center_x(), transformed_bounds.top),
            Point::new(transformed_bounds.center_x(), transformed_bounds.bottom),
        ),
        colors.as_slice(),
        None,
        TileMode::Clamp,
        None,
        None,
    )
    .expect("Failed to create gradient shader");
    paint.set_shader(shader);

    canvas.draw_path(&path, &paint);
}

fn draw_overlay_badge(canvas: &Canvas, size: i32) {
    let badge_x = size as f32 * 0.78;
    let badge_y = size as f32 * 0.78;
    let radius = size as f32 * 0.14;

    let mut badge_paint = Paint::default();
    badge_paint.set_anti_alias(true);
    badge_paint.set_color(Color::from_rgb(45, 45, 45));
    canvas.draw_circle((badge_x, badge_y), radius, &badge_paint);

    let mut icon_paint = Paint::default();
    icon_paint.set_anti_alias(true);
    icon_paint.set_color(Color::WHITE);
    icon_paint.set_style(PaintStyle::Stroke);
    icon_paint.set_stroke_width(radius * 0.12);

    let r = radius * 0.5;
    let gap = r * 0.45;
    icon_paint.set_stroke_width(radius * 0.15);

    let mut path = Path::new();
    path.move_to((badge_x - gap, badge_y - r * 0.6));
    path.line_to((badge_x - gap - r * 0.6, badge_y));
    path.line_to((badge_x - gap, badge_y + r * 0.6));
    path.move_to((badge_x + gap, badge_y - r * 0.6));
    path.line_to((badge_x + gap + r * 0.6, badge_y));
    path.line_to((badge_x + gap, badge_y + r * 0.6));
    canvas.draw_path(&path, &icon_paint);
}
