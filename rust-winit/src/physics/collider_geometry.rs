use super::{
    HORSE_HALF_HEIGHT, HORSE_HALF_WIDTH, HORSE_SUPPORT_HALF_HEIGHT, StackObjectKind,
    stack_object_profile,
};

/// Return area-centred convex collider polygons, or `None` for the circular chickens body.
pub fn centred_collider_polygons(kind: StackObjectKind) -> Option<Vec<Vec<f64>>> {
    let polygons = raw_collider_polygons(kind)?;
    let [centroid_x, centroid_y] = collider_centroid(&polygons);
    Some(
        polygons
            .into_iter()
            .map(|points| {
                points
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        value
                            - if index % 2 == 0 {
                                centroid_x
                            } else {
                                centroid_y
                            }
                    })
                    .collect()
            })
            .collect(),
    )
}

pub fn collider_centroid_offset_y(kind: StackObjectKind) -> Option<f64> {
    raw_collider_polygons(kind).map(|polygons| -collider_centroid(&polygons)[1])
}

pub fn polygon_area_centroid(points: &[f64]) -> [f64; 3] {
    let mut twice_area = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    for index in (0..points.len()).step_by(2) {
        let x0 = points[index];
        let y0 = points[index + 1];
        let next = (index + 2) % points.len();
        let x1 = points[next];
        let y1 = points[next + 1];
        let cross = x0 * y1 - x1 * y0;
        twice_area += cross;
        centroid_x += (x0 + x1) * cross;
        centroid_y += (y0 + y1) * cross;
    }

    if twice_area == 0.0 {
        return [0.0, 0.0, 0.0];
    }
    let scale = 1.0 / (3.0 * twice_area);
    [twice_area / 2.0, centroid_x * scale, centroid_y * scale]
}

pub fn horse_collider_polygons() -> Vec<Vec<f64>> {
    let width = HORSE_HALF_WIDTH;
    let height = HORSE_HALF_HEIGHT;
    let back_y = HORSE_SUPPORT_HALF_HEIGHT;
    vec![
        box_points(-0.667 * width, -height, 0.926 * width, back_y),
        box_points(-0.944 * width, back_y, -0.278 * width, height),
    ]
}

pub fn box_points(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Vec<f64> {
    vec![min_x, min_y, max_x, min_y, max_x, max_y, min_x, max_y]
}

pub fn polygon_collider_points(kind: StackObjectKind) -> Option<Vec<f64>> {
    let profile = stack_object_profile(kind);
    let height = profile.half_height;
    let width = profile.half_width;
    match kind {
        StackObjectKind::Horse => Some(vec![
            -width * 0.95,
            -height * 0.24,
            -width * 0.675,
            -height * 0.853,
            -width * 0.2,
            -height,
            width * 0.65,
            -height * 0.853,
            width,
            height * 0.118,
            width * 0.75,
            height,
            -width * 0.7,
            height * 0.941,
            -width,
            height * 0.235,
        ]),
        StackObjectKind::Hay => Some(box_points(-width, -height, width, height)),
        StackObjectKind::Cow => Some(vec![
            -width,
            -height * 0.65,
            -width * 0.72,
            -height,
            width * 0.72,
            -height,
            width,
            -height * 0.25,
            width * 0.82,
            height * 0.88,
            -width * 0.75,
            height,
        ]),
        StackObjectKind::Chickens => None,
    }
}

fn raw_collider_polygons(kind: StackObjectKind) -> Option<Vec<Vec<f64>>> {
    if kind == StackObjectKind::Horse {
        return Some(horse_collider_polygons());
    }
    polygon_collider_points(kind).map(|points| vec![points])
}

fn collider_centroid(polygons: &[Vec<f64>]) -> [f64; 2] {
    let mut total_area = 0.0;
    let mut centroid_x = 0.0;
    let mut centroid_y = 0.0;

    for points in polygons {
        let [area, x, y] = polygon_area_centroid(points);
        total_area += area;
        centroid_x += area * x;
        centroid_y += area * y;
    }

    if total_area == 0.0 {
        return [0.0, 0.0];
    }
    [centroid_x / total_area, centroid_y / total_area]
}

#[cfg(test)]
mod tests {
    use super::{centred_collider_polygons, polygon_area_centroid};
    use crate::physics::StackObjectKind;

    #[test]
    fn rectangle_centroid_is_its_geometric_middle() {
        let [area, x, y] = polygon_area_centroid(&[0.0, 0.0, 4.0, 0.0, 4.0, 2.0, 0.0, 2.0]);
        assert_eq!(area, 8.0);
        assert!((x - 2.0).abs() < 1e-12);
        assert!((y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn centred_horse_compound_has_zero_area_centroid() {
        let polygons = centred_collider_polygons(StackObjectKind::Horse).unwrap();
        let mut total_area = 0.0;
        let mut weighted_x = 0.0;
        let mut weighted_y = 0.0;
        for polygon in polygons {
            let [area, x, y] = polygon_area_centroid(&polygon);
            total_area += area;
            weighted_x += area * x;
            weighted_y += area * y;
        }
        assert!((weighted_x / total_area).abs() < 1e-12);
        assert!((weighted_y / total_area).abs() < 1e-12);
    }

    #[test]
    fn chickens_keep_their_circle_collider() {
        assert!(centred_collider_polygons(StackObjectKind::Chickens).is_none());
    }
}
