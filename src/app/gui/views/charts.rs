use plotters::prelude::*;

pub fn render_line_chart_svg(
    values: &[f64],
    y_label: &str,
    line_color: RGBColor,
    point_color: RGBColor,
) -> Option<String> {
    if values.is_empty() {
        return None;
    }

    let width = 720;
    let height = 220;
    let x_max = values.len().saturating_sub(1).max(1);
    let y_max = values.iter().copied().fold(0.0_f64, f64::max).max(1.0) * 1.10;

    let mut svg = String::new();
    {
        let backend = SVGBackend::with_string(&mut svg, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&RGBColor(248, 250, 252)).ok()?;

        let mut chart = ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(24)
            .y_label_area_size(44)
            .build_cartesian_2d(0usize..x_max, 0f64..y_max)
            .ok()?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .light_line_style(RGBColor(219, 227, 236))
            .axis_style(RGBColor(100, 116, 139))
            .y_desc(y_label)
            .x_desc("samples")
            .label_style(
                ("sans-serif", 12)
                    .into_font()
                    .color(&RGBColor(100, 116, 139)),
            )
            .draw()
            .ok()?;

        chart
            .draw_series(LineSeries::new(
                values
                    .iter()
                    .enumerate()
                    .map(|(idx, sample)| (idx, *sample)),
                &line_color,
            ))
            .ok()?;

        chart
            .draw_series(
                values
                    .iter()
                    .enumerate()
                    .map(|(idx, sample)| Circle::new((idx, *sample), 2, point_color.filled())),
            )
            .ok()?;

        root.present().ok()?;
    }

    let svg = svg.replacen(
        "<svg ",
        &format!(
            "<svg viewBox=\"0 0 {width} {height}\" preserveAspectRatio=\"none\" style=\"width:100%;height:100%;display:block;\" "
        ),
        1,
    );

    Some(svg)
}
