use eframe::egui::{self, pos2};

/// Appends a squarified layout for the nonzero, descending-weight items.
/// Coordinates are calculated in f64 and rounded only at the egui boundary;
/// sufficiently small rectangles may therefore collapse to zero pixels.
pub(crate) fn squarify(
    items: &[(Option<usize>, u64, usize)],
    rect: egui::Rect,
    out: &mut Vec<(Option<usize>, u64, usize, egui::Rect)>,
) {
    // The input is sorted, so all zero weights form a suffix. A u128 sum is
    // exact even when a slice contains as many u64 weights as usize permits.
    let count = items.iter().take_while(|item| item.1 != 0).count();
    let mut remaining: u128 = items[..count].iter().map(|item| item.1 as u128).sum();
    if remaining == 0 {
        return;
    }

    let (mut left, mut top) = (rect.min.x as f64, rect.min.y as f64);
    let (right, bottom) = (rect.max.x as f64, rect.max.y as f64);
    if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite()
        || right <= left || bottom <= top
    {
        let origin = if rect.min.x.is_finite() && rect.min.y.is_finite() {
            rect.min
        } else {
            egui::Pos2::ZERO
        };
        let empty = egui::Rect::from_min_max(origin, origin);
        out.extend(items[..count].iter().map(|&(id, weight, depth)| (id, weight, depth, empty)));
        return;
    }

    let mut start = 0;
    while start < count {
        let width = right - left;
        let height = bottom - top;
        if width <= 0.0 || height <= 0.0 {
            let point = pos2(left as f32, top as f32);
            let empty = egui::Rect::from_min_max(point, point);
            out.extend(items[start..count].iter().map(|&(id, weight, depth)| (id, weight, depth, empty)));
            break;
        }

        let short = width.min(height);
        let area_per_weight = (width * height) / remaining as f64;
        let largest = items[start].1 as f64 * area_per_weight;
        let mut row_weight = items[start].1 as u128;
        let mut end = start + 1;
        let mut worst = worst_ratio(row_weight as f64 * area_per_weight, largest, largest, short);
        while end < count {
            let next_weight = row_weight + items[end].1 as u128;
            let next_area = items[end].1 as f64 * area_per_weight;
            let next_worst = worst_ratio(next_weight as f64 * area_per_weight, largest, next_area, short);
            if next_worst > worst {
                break;
            }
            row_weight = next_weight;
            worst = next_worst;
            end += 1;
        }

        let vertical = width >= height;
        let fraction = row_weight as f64 / remaining as f64;
        let edge = if vertical {
            if row_weight == remaining { right } else { (left + width * fraction).min(right) }
        } else if row_weight == remaining {
            bottom
        } else {
            (top + height * fraction).min(bottom)
        };
        let mut offset = if vertical { top } else { left };
        let mut row_used = 0u128;
        for &(id, weight, depth) in &items[start..end] {
            row_used += weight as u128;
            let next = if row_used == row_weight {
                if vertical { bottom } else { right }
            } else if vertical {
                top + height * (row_used as f64 / row_weight as f64)
            } else {
                left + width * (row_used as f64 / row_weight as f64)
            };
            let tile = if vertical {
                egui::Rect::from_min_max(pos2(left as f32, offset as f32), pos2(edge as f32, next as f32))
            } else {
                egui::Rect::from_min_max(pos2(offset as f32, top as f32), pos2(next as f32, edge as f32))
            };
            out.push((id, weight, depth, tile));
            offset = next;
        }
        if vertical { left = edge; } else { top = edge; }
        remaining -= row_weight;
        start = end;
    }
}

fn worst_ratio(row_area: f64, largest: f64, smallest: f64, short: f64) -> f64 {
    let row_squared = row_area * row_area;
    let short_squared = short * short;
    (largest * short_squared / row_squared).max(row_squared / (smallest * short_squared))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_are_proportional_ordered_and_nonoverlapping() {
        let items = [(Some(1), 6, 0), (Some(2), 3, 1), (None, 1, 2), (None, 0, 3)];
        let rect = egui::Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 70.0));
        let mut out = vec![(None, 99, 99, rect)];
        squarify(&items, rect, &mut out);
        assert_eq!(out.len(), 4);
        for (index, tile) in out[1..].iter().enumerate() {
            assert_eq!((tile.0, tile.1, tile.2), items[index]);
            assert!((tile.3.area() - rect.area() * tile.1 as f32 / 10.0).abs() < 0.01);
            assert!(tile.3.min.x >= rect.min.x && tile.3.max.x <= rect.max.x);
            assert!(tile.3.min.y >= rect.min.y && tile.3.max.y <= rect.max.y);
            for other in &out[1..index + 1] {
                let overlap_width = (tile.3.max.x.min(other.3.max.x) - tile.3.min.x.max(other.3.min.x)).max(0.0);
                let overlap_height = (tile.3.max.y.min(other.3.max.y) - tile.3.min.y.max(other.3.min.y)).max(0.0);
                assert_eq!(overlap_width * overlap_height, 0.0);
            }
        }
    }

    #[test]
    fn handles_empty_and_extreme_inputs() {
        let mut out = Vec::new();
        let rect = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        squarify(&[], rect, &mut out);
        squarify(&[(None, 0, 0)], rect, &mut out);
        assert!(out.is_empty());
        squarify(&[(Some(0), u64::MAX, 0), (Some(1), 1, 1)], rect, &mut out);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|tile| tile.3.min.x.is_finite() && tile.3.max.x.is_finite()));
        let mut empty = Vec::new();
        squarify(&[(None, 1, 0)], egui::Rect::from_min_max(pos2(2.0, 3.0), pos2(2.0, 3.0)), &mut empty);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].3.area(), 0.0);
    }
}
