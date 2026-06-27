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

pub fn render_time_axis_chart_svg(
    points: &[(i64, f64, bool)],
    y_label: &str,
    line_color: RGBColor,
    point_color: RGBColor,
    tz_offset_hours: i8,
) -> Option<String> {
    if points.is_empty() {
        return None;
    }

    let cache_key =
        compute_chart_cache_key(points, y_label, line_color, point_color, tz_offset_hours);
    if let Some(svg) = chart_cache_get(cache_key) {
        return Some(svg);
    }

    let width = 760u32;
    let height = 240u32;
    let first_ms = points.first().map(|p| p.0)?;
    let last_ms = points.last().map(|p| p.0)?;
    let to_x = |ms: i64| -> f64 { (ms - first_ms) as f64 / 1000.0 };
    let x_max = to_x(last_ms).max(1.0);
    let y_vals: Vec<f64> = points.iter().map(|p| p.1).collect();
    let y_max = y_vals.iter().copied().fold(0.0_f64, f64::max).max(1.0) * 1.10;
    let (peak_idx, peak_value) = y_vals
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or((0, 0.0));
    let last_idx = y_vals.len().saturating_sub(1);
    let last_value = y_vals[last_idx];

    let red = RGBColor(239, 68, 68);

    let mut svg = String::new();
    {
        let backend = SVGBackend::with_string(&mut svg, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&RGBColor(248, 250, 252)).ok()?;

        let mut chart = ChartBuilder::on(&root)
            .margin(12)
            .x_label_area_size(26)
            .y_label_area_size(46)
            .build_cartesian_2d(0f64..x_max, 0f64..y_max)
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
            .x_desc("time")
            .label_style(
                ("sans-serif", 12)
                    .into_font()
                    .color(&RGBColor(100, 116, 139)),
            )
            .x_label_formatter(&|seconds| {
                let ms = first_ms.saturating_add((seconds * 1000.0) as i64);
                format_hhmmss(ms, tz_offset_hours)
            })
            .draw()
            .ok()?;

        chart
            .draw_series(AreaSeries::new(
                points.iter().map(|(ts, v, _)| (to_x(*ts), *v)),
                0.0,
                line_color.mix(0.16),
            ))
            .ok()?;

        let flagged_runs = time_axis_flag_segments(points);

        for (seg_start, seg_end) in &flagged_runs {
            chart
                .draw_series(AreaSeries::new(
                    (*seg_start..=*seg_end).map(|i| (to_x(points[i].0), points[i].1)),
                    0.0,
                    red.mix(0.22),
                ))
                .ok()?;
        }

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|(ts, v, _)| (to_x(*ts), *v)),
                ShapeStyle::from(&line_color).stroke_width(3),
            ))
            .ok()?;

        for (seg_start, seg_end) in &flagged_runs {
            chart
                .draw_series(LineSeries::new(
                    (*seg_start..=*seg_end).map(|i| (to_x(points[i].0), points[i].1)),
                    ShapeStyle::from(&red).stroke_width(4),
                ))
                .ok()?;
        }

        chart
            .draw_series(std::iter::once(Circle::new(
                (to_x(points[peak_idx].0), peak_value),
                4,
                RGBColor(180, 83, 9).filled(),
            )))
            .ok()?;

        chart
            .draw_series(std::iter::once(Circle::new(
                (to_x(points[last_idx].0), last_value),
                5,
                point_color.filled(),
            )))
            .ok()?;

        // one marker per anomaly run at its peak, not one dot per flagged sample.
        chart
            .draw_series(flagged_runs.iter().map(|(seg_start, seg_end)| {
                let mut peak = *seg_start;
                let mut peak_y = points[peak].1;
                for i in (*seg_start)..=(*seg_end) {
                    if points[i].1 > peak_y {
                        peak_y = points[i].1;
                        peak = i;
                    }
                }
                Circle::new((to_x(points[peak].0), peak_y), 5, red.filled())
            }))
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

    chart_cache_put(cache_key, svg.clone());
    Some(svg)
}

// module-level chart cache, drop-everything-when-full eviction
const CHART_CACHE_MAX_ENTRIES: usize = 32;

fn chart_cache() -> &'static std::sync::Mutex<std::collections::HashMap<u64, String>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u64, String>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn chart_cache_get(key: u64) -> Option<String> {
    chart_cache().lock().ok()?.get(&key).cloned()
}

fn chart_cache_put(key: u64, svg: String) {
    let Ok(mut guard) = chart_cache().lock() else {
        return;
    };
    if guard.len() >= CHART_CACHE_MAX_ENTRIES {
        guard.clear();
    }
    guard.insert(key, svg);
}

fn compute_chart_cache_key(
    points: &[(i64, f64, bool)],
    y_label: &str,
    line_color: RGBColor,
    point_color: RGBColor,
    tz_offset_hours: i8,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    y_label.hash(&mut h);
    line_color.0.hash(&mut h);
    line_color.1.hash(&mut h);
    line_color.2.hash(&mut h);
    point_color.0.hash(&mut h);
    point_color.1.hash(&mut h);
    point_color.2.hash(&mut h);
    tz_offset_hours.hash(&mut h);
    points.len().hash(&mut h);
    for (ts, v, flag) in points {
        ts.hash(&mut h);
        // f64 has no Hash impl, so hash the bit pattern.
        v.to_bits().hash(&mut h);
        flag.hash(&mut h);
    }
    h.finish()
}

fn format_hhmmss(ms: i64, tz_offset_hours: i8) -> String {
    use std::time::{Duration, SystemTime};
    use time::{OffsetDateTime, UtcOffset};
    let Ok(millis) = u64::try_from(ms) else {
        return "n/a".to_string();
    };
    let ts = SystemTime::UNIX_EPOCH + Duration::from_millis(millis);
    let offset = UtcOffset::from_hms(tz_offset_hours, 0, 0).unwrap_or(UtcOffset::UTC);
    let dt = OffsetDateTime::from(ts).to_offset(offset);
    format!("{:02}:{:02}:{:02}", dt.hour(), dt.minute(), dt.second())
}

fn time_axis_flag_segments(points: &[(i64, f64, bool)]) -> Vec<(usize, usize)> {
    let mut segments = Vec::new();
    let mut in_segment = false;
    let mut start = 0usize;
    let last_idx = points.len().saturating_sub(1);

    for (i, (_, _, flag)) in points.iter().enumerate() {
        match (*flag, in_segment) {
            (true, false) => {
                in_segment = true;
                start = i;
            }
            (false, true) => {
                in_segment = false;
                segments.push((start.saturating_sub(1), i.min(last_idx)));
            }
            _ => {}
        }
    }
    if in_segment {
        segments.push((start.saturating_sub(1), last_idx));
    }
    segments
}
