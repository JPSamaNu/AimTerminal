//! Colocación de recuadros sin solaparse, compartida por los modos.

use ratatui::layout::Rect;

/// Separación mínima entre recuadros, en celdas.
pub const GAP: u16 = 1;
/// Altura mínima: con 3 filas el número cabe dentro de un recuadro hueco.
pub const MIN_H: u16 = 3;
pub const MIN_W: u16 = 3;

/// Coloca `n` recuadros de `w`x`h` dentro de `area` sin solaparse (con separación `GAP`).
pub fn place_boxes(area: Rect, n: usize, w: u16, h: u16, rng: &mut fastrand::Rng) -> Option<Vec<Rect>> {
    if w > area.width || h > area.height {
        return None;
    }
    for _ in 0..30 {
        let mut placed: Vec<Rect> = Vec::with_capacity(n);
        for _ in 0..n {
            match find_spot(area, &placed, w, h, None, rng) {
                Some(r) => placed.push(r),
                None => break,
            }
        }
        if placed.len() == n {
            return Some(placed);
        }
    }
    None
}

/// Busca un lugar al azar que no choque con `others` ni contenga el punto `avoid`.
pub fn find_spot(
    area: Rect,
    others: &[Rect],
    w: u16,
    h: u16,
    avoid: Option<(f32, f32)>,
    rng: &mut fastrand::Rng,
) -> Option<Rect> {
    if w > area.width || h > area.height {
        return None;
    }
    for _ in 0..200 {
        let x = area.x + rng.u16(0..=area.width - w);
        let y = area.y + rng.u16(0..=area.height - h);
        let r = Rect { x, y, width: w, height: h };
        let free = others.iter().all(|p| !overlaps_with_gap(p, &r));
        let clear = avoid.is_none_or(|(px, py)| !contains(&r, px, py));
        if free && clear {
            return Some(r);
        }
    }
    None
}

pub fn overlaps_with_gap(a: &Rect, b: &Rect) -> bool {
    let ax2 = a.x + a.width + GAP;
    let ay2 = a.y + a.height + GAP;
    let bx2 = b.x + b.width + GAP;
    let by2 = b.y + b.height + GAP;
    a.x < bx2 && b.x < ax2 && a.y < by2 && b.y < ay2
}

/// Intenta el tamaño pedido y lo reduce si no cabe (paneles pequeños).
pub fn layout(area: Rect, n: usize, w: u16, h: u16, rng: &mut fastrand::Rng) -> Vec<Rect> {
    let (mut w, mut h, mut n) = (w, h, n);
    loop {
        if let Some(v) = place_boxes(area, n, w, h, rng) {
            return v;
        }
        if w > MIN_W {
            w -= 1;
        } else if h > MIN_H {
            h -= 1;
        } else if n > 2 {
            n -= 1;
        } else {
            return place_boxes(area, 1, w.min(area.width), h.min(area.height), rng).unwrap_or_default();
        }
    }
}

/// El área con un margen de una celda.
pub fn inner(area: Rect) -> Rect {
    if area.width > 2 && area.height > 2 {
        Rect { x: area.x + 1, y: area.y + 1, width: area.width - 2, height: area.height - 2 }
    } else {
        area
    }
}

pub fn center(r: &Rect) -> (f32, f32) {
    (r.x as f32 + r.width as f32 / 2.0, r.y as f32 + r.height as f32 / 2.0)
}

pub fn contains(r: &Rect, x: f32, y: f32) -> bool {
    x >= r.x as f32 && x < (r.x + r.width) as f32 && y >= r.y as f32 && y < (r.y + r.height) as f32
}

/// Distancia del cursor a un punto, en anchos de celda; la vertical cuenta doble
/// porque una celda mide más o menos el doble de alto que de ancho.
pub fn cell_distance(from: (f32, f32), to: (f32, f32)) -> f32 {
    ((to.0 - from.0).powi(2) + (2.0 * (to.1 - from.1)).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: u16, h: u16) -> Rect {
        Rect { x: 0, y: 0, width: w, height: h }
    }

    #[test]
    fn boxes_never_overlap_and_stay_inside() {
        let mut rng = fastrand::Rng::with_seed(7);
        for seed in 0..300 {
            rng.seed(seed);
            let a = area(rng.u16(40..=200), rng.u16(11..=60));
            let n = rng.usize(3..=5);
            let boxes = layout(a, n, 10, 5, &mut rng);
            assert!(!boxes.is_empty());
            for (i, b) in boxes.iter().enumerate() {
                assert!(b.x + b.width <= a.width && b.y + b.height <= a.height, "fuera: {b:?} en {a:?}");
                for c in &boxes[i + 1..] {
                    assert!(!overlaps_with_gap(b, c), "solapados {b:?} {c:?}");
                }
            }
        }
    }

    #[test]
    fn small_panel_shrinks_instead_of_failing() {
        let mut rng = fastrand::Rng::with_seed(1);
        let boxes = layout(area(38, 9), 5, 10, 5, &mut rng);
        assert_eq!(boxes.len(), 5);
    }

    #[test]
    fn gap_counts_as_overlap() {
        let a = Rect { x: 0, y: 0, width: 4, height: 3 };
        let touching = Rect { x: 4, y: 0, width: 4, height: 3 };
        let apart = Rect { x: 5, y: 0, width: 4, height: 3 };
        assert!(overlaps_with_gap(&a, &touching));
        assert!(!overlaps_with_gap(&a, &apart));
    }

    #[test]
    fn find_spot_respects_others_and_point() {
        let mut rng = fastrand::Rng::with_seed(5);
        let a = area(60, 20);
        let others = vec![Rect::new(2, 2, 6, 3), Rect::new(30, 10, 6, 3)];
        for _ in 0..500 {
            let r = find_spot(a, &others, 6, 3, Some((20.5, 5.5)), &mut rng).unwrap();
            assert!(others.iter().all(|o| !overlaps_with_gap(o, &r)));
            assert!(!contains(&r, 20.5, 5.5));
        }
    }

    #[test]
    fn distance_doubles_vertical() {
        assert_eq!(cell_distance((0.0, 0.0), (3.0, 2.0)), 5.0);
    }
}
