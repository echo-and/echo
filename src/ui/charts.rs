use gpui::*;

#[derive(Clone)]
pub(crate) struct RealtimeSeries {
    values: Vec<f64>,
    stroke: Hsla,
    fill: Background,
}

impl RealtimeSeries {
    pub(crate) fn new(
        values: impl IntoIterator<Item = f64>,
        stroke: Hsla,
        fill: impl Into<Background>,
    ) -> Self {
        Self {
            values: values
                .into_iter()
                .map(|value| if value.is_finite() { value.max(0.) } else { 0. })
                .collect(),
            stroke,
            fill: fill.into(),
        }
    }
}

pub(crate) fn realtime_area_chart(series: Vec<RealtimeSeries>) -> impl IntoElement {
    RealtimeAreaChart { series }
}

struct RealtimeAreaChart {
    series: Vec<RealtimeSeries>,
}

impl IntoElement for RealtimeAreaChart {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RealtimeAreaChart {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            size: Size::full(),
            ..Default::default()
        };

        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        _: &mut App,
    ) {
        let max_value = max_series_value(&self.series);
        let domain_max = if max_value > 0. { max_value * 1.1 } else { 1. };

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for series in &self.series {
                paint_series(series, bounds, domain_max, window);
            }
        });
    }
}

fn max_series_value(series: &[RealtimeSeries]) -> f64 {
    series
        .iter()
        .flat_map(|series| series.values.iter().copied())
        .fold(0., f64::max)
}

fn paint_series(
    series: &RealtimeSeries,
    bounds: Bounds<Pixels>,
    domain_max: f64,
    window: &mut Window,
) {
    if series.values.is_empty() {
        return;
    }

    let points = series_points(&series.values, bounds, domain_max);
    if points.is_empty() {
        return;
    }

    if let Some(area) = area_path(&points, bounds.bottom()) {
        window.paint_path(area, series.fill);
    }
    if let Some(line) = line_path(&points) {
        window.paint_path(line, series.stroke);
    }
}

fn series_points(values: &[f64], bounds: Bounds<Pixels>, domain_max: f64) -> Vec<Point<Pixels>> {
    let width = bounds.size.width.as_f32().max(0.);
    let height = bounds.size.height.as_f32().max(0.);
    let baseline = bounds.bottom() - px(1.);
    let chart_height = (height - 1.).max(0.);
    let x_step = if values.len() <= 1 {
        0.
    } else {
        width / (values.len() - 1) as f32
    };

    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = if values.len() == 1 {
                bounds.origin.x + px(width / 2.)
            } else {
                bounds.origin.x + px(index as f32 * x_step)
            };
            let ratio = (*value / domain_max).clamp(0., 1.) as f32;
            let y = baseline - px(ratio * chart_height);

            point(x, y)
        })
        .collect()
}

fn area_path(points: &[Point<Pixels>], baseline: Pixels) -> Option<Path<Pixels>> {
    let first = points.first().copied()?;
    let last = points.last().copied()?;
    let mut builder = PathBuilder::fill();

    if points.len() == 1 {
        let half_width = px(1.);
        builder.move_to(point(first.x - half_width, first.y));
        builder.line_to(point(first.x + half_width, first.y));
        builder.line_to(point(first.x + half_width, baseline));
        builder.line_to(point(first.x - half_width, baseline));
        builder.close();
        return builder.build().ok();
    }

    builder.move_to(first);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.line_to(point(last.x, baseline));
    builder.line_to(point(first.x, baseline));
    builder.close();
    builder.build().ok()
}

fn line_path(points: &[Point<Pixels>]) -> Option<Path<Pixels>> {
    let first = points.first().copied()?;
    let mut builder = PathBuilder::stroke(px(1.));

    if points.len() == 1 {
        builder.move_to(point(first.x - px(2.), first.y));
        builder.line_to(point(first.x + px(2.), first.y));
        return builder.build().ok();
    }

    builder.move_to(first);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.build().ok()
}

#[cfg(test)]
mod tests {
    use gpui::{Bounds, hsla, point, px, size};

    use super::{RealtimeSeries, area_path, line_path, series_points};

    #[test]
    fn clamps_non_finite_values_to_zero() {
        let series = RealtimeSeries::new(
            [1., f64::NAN, f64::INFINITY, -1.],
            hsla(0., 0., 0., 1.),
            hsla(0., 0., 0., 0.),
        );

        assert_eq!(series.values, vec![1., 0., 0., 0.]);
    }

    #[test]
    fn creates_points_for_all_zero_series() {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(40.)));
        let points = series_points(&[0., 0., 0.], bounds, 1.);

        assert_eq!(points.len(), 3);
        assert!(points.iter().all(|point| point.y == px(39.)));
        assert!(line_path(&points).is_some());
        assert!(area_path(&points, bounds.bottom()).is_some());
    }

    #[test]
    fn creates_short_line_for_single_point() {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(40.)));
        let points = series_points(&[5.], bounds, 10.);

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].x, px(50.));
        assert!(points[0].y > px(0.));
        assert!(points[0].y < bounds.bottom());
        assert!(line_path(&points).is_some());
        assert!(area_path(&points, bounds.bottom()).is_some());
    }
}
