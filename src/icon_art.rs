// The program's icon, drawn by code: the mark from the editor's top-left corner (a ring with a
// hand at twelve) on a dark rounded square. build.rs includes this file to put the icon inside
// the program file, and the program uses it for its windows, so there is one drawing and no
// picture file to keep in step. (Plain comments: build.rs takes this file in with `include!`.)

/// Samples along each pixel edge: 4 x 4 per pixel gives smooth edges at every size.
const SUB: u32 = 4;
const BACK: [f32; 3] = [23.0, 28.0, 36.0];
const MARK: [f32; 3] = [132.0, 226.0, 190.0];
// Sizes as parts of the icon's width, from its centre.
const HALF: f32 = 0.47;
const CORNER: f32 = 0.12;
const RING: f32 = 0.29;
const RING_WIDTH: f32 = 0.095;
const HAND: f32 = 0.17;
const HAND_WIDTH: f32 = 0.075;

fn in_back(x: f32, y: f32) -> bool {
    let (dx, dy) = ((x.abs() - (HALF - CORNER)).max(0.0), (y.abs() - (HALF - CORNER)).max(0.0));
    x.abs() <= HALF && y.abs() <= HALF && dx * dx + dy * dy <= CORNER * CORNER
}

fn in_mark(x: f32, y: f32) -> bool {
    let on_ring = ((x * x + y * y).sqrt() - RING).abs() <= RING_WIDTH / 2.0;
    // The hand: a round-ended line from the centre straight up.
    let dy = y + (-y).clamp(0.0, HAND);
    on_ring || x * x + dy * dy <= HAND_WIDTH * HAND_WIDTH / 4.0
}

/// The icon at `size` x `size` pixels: RGBA, straight alpha, top row first.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut back, mut mark) = (0u32, 0u32);
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let at = |pixel: u32, sample: u32| (pixel as f32 + (sample as f32 + 0.5) / SUB as f32) / size as f32 - 0.5;
                    let (px, py) = (at(x, sx), at(y, sy));
                    if in_mark(px, py) {
                        mark += 1;
                    } else if in_back(px, py) {
                        back += 1;
                    }
                }
            }
            let covered = (back + mark) as f32;
            if covered == 0.0 {
                out.extend([0, 0, 0, 0]);
                continue;
            }
            for c in 0..3 {
                out.push(((BACK[c] * back as f32 + MARK[c] * mark as f32) / covered).round() as u8);
            }
            out.push((255.0 * covered / (SUB * SUB) as f32).round() as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_a_ring_with_a_hand_on_a_rounded_square() {
        for size in [16u32, 32, 64, 256] {
            let rgba = icon_rgba(size);
            assert_eq!(rgba.len(), (size * size * 4) as usize);
            let at = |x: f32, y: f32| {
                let (px, py) = (((x + 0.5) * size as f32) as u32, ((y + 0.5) * size as f32) as u32);
                let i = ((py.min(size - 1) * size + px.min(size - 1)) * 4) as usize;
                [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
            };
            assert_eq!(at(-0.5, -0.5)[3], 0, "{size}: the corner is see-through");
            assert_eq!(at(0.4, 0.0), [23, 28, 36, 255], "{size}: dark inside the square");
            assert_eq!(at(0.0, -RING), [132, 226, 190, 255], "{size}: the ring at twelve");
            assert_eq!(at(RING, 0.0), [132, 226, 190, 255], "{size}: the ring at three");
            if size >= 64 {
                // Thinner than a pixel of the small sizes.
                assert_eq!(at(0.0, -HAND / 2.0), [132, 226, 190, 255], "{size}: the hand");
                assert_eq!(at(0.0, HAND), [23, 28, 36, 255], "{size}: no hand below the centre");
            }
        }
    }
}
