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

    let width = 760;
    let height = 240;
    let x_max = values.len().saturating_sub(1).max(1);
    let y_max = values.iter().copied().fold(0.0_f64, f64::max).max(1.0) * 1.10;
    let (peak_idx, peak_value) = values
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, 0.0));
    let last_idx = values.len().saturating_sub(1);
    let last_value = values[last_idx];

    let mut svg = String::new();
    {
        let backend = SVGBackend::with_string(&mut svg, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&RGBColor(248, 250, 252)).ok()?;

        let mut chart = ChartBuilder::on(&root)
            .margin(12)
            .x_label_area_size(26)
            .y_label_area_size(46)
            .build_cartesian_2d(0usize..x_max, 0f64..y_max)
            .ok()?;

        chart
            .configure_mesh()
            .x_labels(6)
            .y_labels(5)
            .x_label_style(
                ("sans-serif", 11)
                    .into_font()
                    .color(&RGBColor(100, 116, 139)),
            )
            .y_label_style(
                ("sans-serif", 11)
                    .into_font()
                    .color(&RGBColor(100, 116, 139)),
            )
            .light_line_style(RGBColor(226, 232, 240))
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
            .draw_series(AreaSeries::new(
                values
                    .iter()
                    .enumerate()
                    .map(|(idx, sample)| (idx, *sample)),
                0.0,
                line_color.mix(0.16),
            ))
            .ok()?;

        chart
            .draw_series(LineSeries::new(
                values
                    .iter()
                    .enumerate()
                    .map(|(idx, sample)| (idx, *sample)),
                ShapeStyle::from(&line_color).stroke_width(3),
            ))
            .ok()?;

        chart
            .draw_series(std::iter::once(Circle::new(
                (peak_idx, peak_value),
                4,
                RGBColor(180, 83, 9).filled(),
            )))
            .ok()?;

        chart
            .draw_series(std::iter::once(Circle::new(
                (last_idx, last_value),
                5,
                point_color.filled(),
            )))
            .ok()?;

        root.present().ok()?;
    }

    let svg = svg.replacen(
        "<svg ",
        &format!(
            "<svg viewBox=\"0 0 {width} {height}\" preserveAspectRatio=\"xMidYMid meet\" style=\"width:100%;height:100%;display:block;\" "
        ),
        1,
    );

    Some(svg)
}
