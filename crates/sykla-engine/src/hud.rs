use crate::mesh::Vertex;
use crate::physics::PhysicsState;

/// Glyph bitmap: 7 rows, each row is 5 bits (bit 4 = leftmost column).
fn glyph_bitmap(c: char) -> [u8; 7] {
    match c {
        '0' => [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F],
        '1' => [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x1F, 0x01, 0x01, 0x1F, 0x10, 0x10, 0x1F],
        '3' => [0x1F, 0x01, 0x01, 0x1F, 0x01, 0x01, 0x1F],
        '4' => [0x11, 0x11, 0x11, 0x1F, 0x01, 0x01, 0x01],
        '5' => [0x1F, 0x10, 0x10, 0x1F, 0x01, 0x01, 0x1F],
        '6' => [0x1F, 0x10, 0x10, 0x1F, 0x11, 0x11, 0x1F],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x04, 0x04, 0x04],
        '8' => [0x1F, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x1F],
        '9' => [0x1F, 0x11, 0x11, 0x1F, 0x01, 0x01, 0x1F],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0F, 0x10, 0x10, 0x10, 0x10, 0x10, 0x0F],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0F, 0x10, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x0A, 0x04, 0x04, 0x04, 0x0A, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x06],
        ':' => [0x00, 0x06, 0x06, 0x00, 0x06, 0x06, 0x00],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '-' => [0x00, 0x00, 0x00, 0x0E, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '%' => [0x19, 0x19, 0x02, 0x04, 0x08, 0x13, 0x13],
        '\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        ' ' => [0x00; 7],
        _ => [0x00; 7],
    }
}

fn push_quad(
    verts: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) {
    let base = verts.len() as u32;
    let normal = [r, g, b];
    let uv = [a, 0.0];
    verts.push(Vertex { position: [x, y, 0.0], normal, uv });
    verts.push(Vertex { position: [x + w, y, 0.0], normal, uv });
    verts.push(Vertex { position: [x + w, y + h, 0.0], normal, uv });
    verts.push(Vertex { position: [x, y + h, 0.0], normal, uv });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn push_text(
    verts: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    text: &str,
    x: f32,
    y: f32,
    char_w: f32,
    char_h: f32,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) {
    let cell_w = char_w / 5.0;
    let cell_h = char_h / 7.0;
    let spacing = char_w * 1.2;

    for (ci, c) in text.chars().enumerate() {
        let cx = x + ci as f32 * spacing;
        let bitmap = glyph_bitmap(c);

        for (row, &bits) in bitmap.iter().enumerate() {
            let ry = y + row as f32 * cell_h;
            let mut col = 0u32;
            while col < 5 {
                if bits & (1 << (4 - col)) != 0 {
                    let start = col;
                    while col < 5 && bits & (1 << (4 - col)) != 0 {
                        col += 1;
                    }
                    let run_len = col - start;
                    push_quad(
                        verts,
                        indices,
                        cx + start as f32 * cell_w,
                        ry,
                        run_len as f32 * cell_w,
                        cell_h,
                        r,
                        g,
                        b,
                        a,
                    );
                } else {
                    col += 1;
                }
            }
        }
    }
}

/// Returns the pixel width of a text string at the given char_w.
fn text_width(text: &str, char_w: f32) -> f32 {
    let n = text.len() as f32;
    if n <= 0.0 {
        return 0.0;
    }
    // spacing = char_w * 1.2, last char has no trailing gap
    (n - 1.0) * char_w * 1.2 + char_w
}

fn gradient_color(pct: f32) -> (f32, f32, f32) {
    let abs_pct = pct.abs();
    if abs_pct < 2.0 {
        (0.3, 0.9, 0.3)
    } else if abs_pct < 5.0 {
        (0.9, 0.9, 0.2)
    } else {
        (0.95, 0.3, 0.2)
    }
}

/// Route selector screen — full-screen overlay for choosing a route before riding.
pub fn build_selector_hud(
    routes: &[(&str, &str)], // (name, description) pairs
    selected: usize,
    width: f32,
    height: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::with_capacity(16384);
    let mut indices = Vec::with_capacity(32768);

    let scale = (width / 1440.0).clamp(0.5, 2.0);

    // Full-screen dark background
    push_quad(
        &mut verts, &mut indices,
        0.0, 0.0, width, height,
        0.03, 0.03, 0.05, 0.95,
    );

    // Title
    let title = "SELECT ROUTE";
    let title_size = 18.0 * scale;
    let title_tw = text_width(title, title_size);
    let title_x = (width - title_tw) * 0.5;
    let title_y = 40.0 * scale;

    push_text(
        &mut verts, &mut indices,
        title, title_x, title_y,
        title_size, title_size * 1.4,
        0.25, 0.55, 0.95, 1.0,
    );

    // Route list
    let list_top = title_y + title_size * 2.5;
    let row_h = 32.0 * scale;
    let name_size = 8.0 * scale;
    let desc_size = 5.0 * scale;
    let list_w = 500.0 * scale;
    let list_x = (width - list_w) * 0.5;
    let padding = 12.0 * scale;

    // Scroll: compute how many rows fit and the scroll offset
    let hint_y_reserved = 50.0 * scale;
    let list_avail_h = height - list_top - hint_y_reserved;
    let max_visible = (list_avail_h / row_h).floor() as usize;
    let max_visible = max_visible.max(1);
    let scroll_offset = if routes.len() <= max_visible {
        0
    } else if selected < max_visible / 2 {
        0
    } else if selected + max_visible / 2 >= routes.len() {
        routes.len().saturating_sub(max_visible)
    } else {
        selected.saturating_sub(max_visible / 2)
    };
    let visible_end = (scroll_offset + max_visible).min(routes.len());

    for i in scroll_offset..visible_end {
        let (name, desc) = routes[i];
        let y = list_top + (i - scroll_offset) as f32 * row_h;
        let is_selected = i == selected;

        // Row background (highlight for selected)
        if is_selected {
            push_quad(
                &mut verts, &mut indices,
                list_x, y, list_w, row_h - 4.0 * scale,
                0.12, 0.18, 0.30, 0.9,
            );
            // Selection indicator
            push_quad(
                &mut verts, &mut indices,
                list_x, y, 4.0 * scale, row_h - 4.0 * scale,
                0.25, 0.55, 0.95, 1.0,
            );
        } else {
            push_quad(
                &mut verts, &mut indices,
                list_x, y, list_w, row_h - 4.0 * scale,
                0.06, 0.06, 0.08, 0.6,
            );
        }

        // Route name
        let name_upper = name.to_uppercase();
        let (nr, ng, nb) = if is_selected {
            (1.0, 1.0, 1.0)
        } else {
            (0.6, 0.6, 0.65)
        };
        push_text(
            &mut verts, &mut indices,
            &name_upper, list_x + padding, y + 4.0 * scale,
            name_size, name_size * 1.4,
            nr, ng, nb, 1.0,
        );

        // Description
        let desc_upper = desc.to_uppercase();
        let (dr, dg, db) = if is_selected {
            (0.5, 0.6, 0.7)
        } else {
            (0.35, 0.35, 0.4)
        };
        push_text(
            &mut verts, &mut indices,
            &desc_upper, list_x + padding, y + 4.0 * scale + name_size * 1.6,
            desc_size, desc_size * 1.4,
            dr, dg, db, 0.9,
        );
    }

    // Scroll indicators
    let indicator_size = 5.0 * scale;
    if scroll_offset > 0 {
        let arrow = "...";
        let aw = text_width(arrow, indicator_size);
        push_text(
            &mut verts, &mut indices,
            arrow, list_x + (list_w - aw) * 0.5, list_top - indicator_size * 1.8,
            indicator_size, indicator_size * 1.4,
            0.4, 0.4, 0.5, 0.6,
        );
    }
    if visible_end < routes.len() {
        let arrow = "...";
        let aw = text_width(arrow, indicator_size);
        let bottom_y = list_top + (visible_end - scroll_offset) as f32 * row_h;
        push_text(
            &mut verts, &mut indices,
            arrow, list_x + (list_w - aw) * 0.5, bottom_y + 2.0 * scale,
            indicator_size, indicator_size * 1.4,
            0.4, 0.4, 0.5, 0.6,
        );
    }

    // Footer hint
    let hint = "ENTER TO RIDE  -  ESC TO RETURN";
    let hint_size = 6.0 * scale;
    let hint_tw = text_width(hint, hint_size);
    let hint_x = (width - hint_tw) * 0.5;
    let hint_y = height - 40.0 * scale;

    push_text(
        &mut verts, &mut indices,
        hint, hint_x, hint_y,
        hint_size, hint_size * 1.4,
        0.35, 0.35, 0.4, 0.7,
    );

    (verts, indices)
}

pub fn build_hud(
    state: &PhysicsState,
    route_length: f32,
    route_name: &str,
    width: f32,
    _height: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::with_capacity(4096);
    let mut indices = Vec::with_capacity(8192);

    let scale = (width / 1440.0).clamp(0.5, 2.0);
    let margin = 12.0 * scale;
    let padding = 14.0 * scale;
    let gap = 8.0 * scale;

    // ================================================================
    // LEFT PANEL — data rows (POWER, SPEED, HR, CAD, DIST, TIME)
    // ================================================================
    let row_height = 30.0 * scale;
    let num_rows = 6u32;
    let panel_w = 200.0 * scale;
    let panel_h = padding * 2.0 + num_rows as f32 * row_height;
    let panel_x = margin;
    let panel_y = margin;

    // Background
    push_quad(
        &mut verts, &mut indices,
        panel_x, panel_y, panel_w, panel_h,
        0.05, 0.05, 0.08, 0.75,
    );

    let label_size = 7.0 * scale;
    let value_size = 13.0 * scale;
    let unit_size = 7.0 * scale;

    let label_x = panel_x + padding;
    let value_x = panel_x + padding + 70.0 * scale;
    let unit_x = panel_x + padding + 145.0 * scale;

    let (mins, secs) = state.elapsed_display();
    let total_km = state.total_distance / 1000.0;

    let power_str = format!("{}", state.power_watts as u32);
    let speed_str = format!("{:.1}", state.speed_kmh());
    let hr_str = format!("{}", state.heart_rate);
    let cad_str = format!("{}", state.cadence);
    let dist_str = format!("{:.1}", total_km);
    let time_str = format!("{}:{:02}", mins, secs);

    let rows: [(&str, &str, &str, f32, f32, f32); 6] = [
        ("POWER", &power_str, "W", 0.95, 0.75, 0.2),
        ("SPEED", &speed_str, "KM/H", 1.0, 1.0, 1.0),
        ("HR", &hr_str, "BPM", 0.95, 0.25, 0.25),
        ("CAD", &cad_str, "RPM", 0.3, 0.85, 0.9),
        ("DIST", &dist_str, "KM", 0.6, 0.6, 0.6),
        ("TIME", &time_str, "", 0.6, 0.6, 0.6),
    ];

    for (i, &(label, value, unit, r, g, b)) in rows.iter().enumerate() {
        let y = panel_y + padding + i as f32 * row_height;

        push_text(
            &mut verts, &mut indices,
            label, label_x, y,
            label_size, label_size * 1.4,
            r * 0.5, g * 0.5, b * 0.5, 0.8,
        );

        push_text(
            &mut verts, &mut indices,
            value, value_x, y,
            value_size, value_size * 1.4,
            r, g, b, 1.0,
        );

        if !unit.is_empty() {
            push_text(
                &mut verts, &mut indices,
                unit, unit_x, y + (value_size - unit_size) * 0.5,
                unit_size, unit_size * 1.4,
                r * 0.4, g * 0.4, b * 0.4, 0.7,
            );
        }
    }

    // ================================================================
    // RIGHT PANEL — large grade display
    // ================================================================
    let grade_panel_w = 140.0 * scale;
    let grade_panel_h = 80.0 * scale;
    let grade_x = width - margin - grade_panel_w;
    let grade_y = margin;

    let gradient_pct = state.gradient_percent();
    let (gr, gg, gb) = gradient_color(gradient_pct);

    // Background with subtle color tint
    push_quad(
        &mut verts, &mut indices,
        grade_x, grade_y, grade_panel_w, grade_panel_h,
        0.05 + gr * 0.05, 0.05 + gg * 0.05, 0.08 + gb * 0.05, 0.75,
    );

    // "GRADE" label (small, dimmed, top-left of panel)
    push_text(
        &mut verts, &mut indices,
        "GRADE",
        grade_x + padding, grade_y + padding * 0.6,
        label_size, label_size * 1.4,
        gr * 0.5, gg * 0.5, gb * 0.5, 0.8,
    );

    // Big percentage number
    let big_size = 22.0 * scale;
    let grad_str = format!("{:.1}%", gradient_pct);
    let grad_tw = text_width(&grad_str, big_size);
    // Center the big number horizontally in the panel
    let grad_text_x = grade_x + (grade_panel_w - grad_tw) * 0.5;
    let grad_text_y = grade_y + grade_panel_h * 0.35;

    push_text(
        &mut verts, &mut indices,
        &grad_str, grad_text_x, grad_text_y,
        big_size, big_size * 1.4,
        gr, gg, gb, 1.0,
    );

    // ================================================================
    // CENTER — route progress bar
    // ================================================================
    let bar_h = 24.0 * scale;
    let bar_x = panel_x + panel_w + gap;
    let bar_right = grade_x - gap;
    let bar_w = (bar_right - bar_x).max(40.0 * scale);
    let bar_y = margin;

    let route_km = route_length / 1000.0;
    let progress = if route_length > 0.0 {
        (state.distance / route_length).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Bar background
    push_quad(
        &mut verts, &mut indices,
        bar_x, bar_y, bar_w, bar_h,
        0.08, 0.08, 0.10, 0.65,
    );

    // Bar fill
    if progress > 0.001 {
        push_quad(
            &mut verts, &mut indices,
            bar_x, bar_y, bar_w * progress, bar_h,
            0.25, 0.55, 0.95, 0.8,
        );
    }

    // Progress text centered in bar
    let pos_km = state.distance / 1000.0;
    let prog_str = format!("{:.1} / {:.1} KM", pos_km, route_km);
    let prog_size = 7.0 * scale;
    let prog_tw = text_width(&prog_str, prog_size);
    let prog_tx = bar_x + (bar_w - prog_tw) * 0.5;
    let prog_ty = bar_y + (bar_h - prog_size * 1.4) * 0.5;

    push_text(
        &mut verts, &mut indices,
        &prog_str, prog_tx, prog_ty,
        prog_size, prog_size * 1.4,
        0.95, 0.95, 0.95, 0.95,
    );

    // ================================================================
    // LEFT PANEL — route name (bottom, small, dimmed, auto-scaled to fit)
    // ================================================================
    if !route_name.is_empty() {
        let name_upper: String = route_name.to_uppercase();
        let max_name_w = panel_w - padding * 2.0;
        // Start at 5.5 and shrink if needed to fit panel
        let mut name_size = 5.5 * scale;
        while text_width(&name_upper, name_size) > max_name_w && name_size > 2.0 * scale {
            name_size -= 0.5 * scale;
        }
        let name_tw = text_width(&name_upper, name_size);
        let name_x = panel_x + (panel_w - name_tw) * 0.5;
        let name_y = panel_y + panel_h + gap * 0.5;

        push_text(
            &mut verts, &mut indices,
            &name_upper, name_x, name_y,
            name_size, name_size * 1.4,
            0.4, 0.4, 0.45, 0.6,
        );
    }

    (verts, indices)
}
