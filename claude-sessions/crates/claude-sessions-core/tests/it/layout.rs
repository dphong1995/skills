use claude_sessions_core::geometry::{Point, Rect, Size};
use claude_sessions_core::layout::{
    self, Anchor, CANVAS, COLLAPSED, EXPANDED_WIDTH, HAnchor, MARGIN, canvas_frame, clamp,
    clamp_drag, drag_origin, is_drag, legacy_offset, migrate, near, offset, origin, pill,
    pill_centre, relative_origin, settled_pill_origin,
};

const SCR: Rect = Rect::new(0.0, 0.0, 1512.0, 982.0);
const EXT: Rect = Rect::new(1512.0, 0.0, 2560.0, 1440.0);

fn anchor(h: HAnchor, up: bool) -> Anchor {
    Anchor { h, up }
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn sz(w: f64, h: f64) -> Size {
    Size::new(w, h)
}

#[test]
fn anchor_near_screen_regions() {
    assert_eq!(
        near(p(1400.0, 950.0), SCR),
        anchor(HAnchor::Trailing, false)
    );
    assert_eq!(near(p(100.0, 50.0), SCR), anchor(HAnchor::Leading, true));
    assert_eq!(near(p(756.0, 950.0), SCR), anchor(HAnchor::Center, false));
    assert_eq!(
        near(p(EXT.min_x() + 900.0, EXT.max_y() - 50.0), EXT),
        anchor(HAnchor::Center, false)
    );
}

#[test]
fn canvas_pill_round_trip() {
    for a in [
        anchor(HAnchor::Leading, false),
        anchor(HAnchor::Center, true),
        anchor(HAnchor::Trailing, true),
    ] {
        let o = p(600.0, 300.0);
        let f = canvas_frame(o, a);
        assert_eq!(pill(f, COLLAPSED, a).origin(), o);
    }
}

#[test]
fn dragged_past_corner_is_clamped_and_expanded_stays_on_screen() {
    let a = anchor(HAnchor::Trailing, false);
    let corner = canvas_frame(clamp(p(1500.0, 975.0), SCR), a);
    // Six idle rows with a reply: header 32 + list padding 14 + 6 * 92 + 5 gaps of 4 (Swift `Layout.expanded`).
    let open = pill(corner, sz(EXPANDED_WIDTH, 618.0), a);
    assert!(SCR.contains_rect(open));
}

#[test]
fn centred_canvas_at_inner_edge_of_middle_third_stays_on_screen() {
    let s = Rect::new(0.0, 0.0, 1280.0, 800.0);
    let centre = p(s.min_x() + s.w / 3.0 + 1.0, 400.0);
    let a = near(centre, s);
    assert_eq!(a.h, HAnchor::Center);
    let o = p(centre.x - COLLAPSED.w / 2.0, centre.y);
    let f = canvas_frame(o, a);
    assert!(f.min_x() >= s.min_x());
    assert!(pill(f, sz(EXPANDED_WIDTH, 400.0), a).min_x() >= s.min_x());
}

#[test]
fn migrate_shifts_by_anchor() {
    let a = |h| Anchor { h, up: false };
    assert_eq!(migrate(1280.0, 232.0, 124.0, a(HAnchor::Trailing)), 1388.0);
    assert_eq!(migrate(600.0, 232.0, 124.0, a(HAnchor::Center)), 654.0);
    assert_eq!(migrate(40.0, 232.0, 124.0, a(HAnchor::Leading)), 40.0);
}

#[test]
fn migrate_no_op_when_width_unchanged() {
    let a = Anchor::default();
    assert_eq!(migrate(1388.0, 124.0, 124.0, a), 1388.0);
}

#[test]
fn shell_constants_follow_the_glass_prototype() {
    assert_eq!(COLLAPSED, sz(126.0, 30.0));
    assert_eq!(layout::PILL_HEIGHT, 30.0);
    assert_eq!(COLLAPSED.h, layout::PILL_HEIGHT);
    assert_eq!(EXPANDED_WIDTH, 480.0);
    assert_eq!(layout::HEADER_HEIGHT, 38.0);
    assert_eq!(layout::PANEL_PAD, 8.0);
}

#[test]
fn collapsed_size() {
    assert_eq!(COLLAPSED, sz(126.0, 30.0));
    assert_eq!(CANVAS, sz(508.0, 640.0));
    assert_eq!(MARGIN, 14.0);
    assert_eq!(layout::SEEN_DWELL_SECS, 1.0);
}

#[test]
fn is_drag_threshold() {
    let o = Point::default();
    assert!(!is_drag(o, p(3.0, 0.0)));
    assert!(!is_drag(o, p(4.0, 0.0)));
    assert!(is_drag(o, p(3.0, 3.0)));
    assert!(is_drag(o, p(5.0, 0.0)));
}

#[test]
fn drag_origin_follows_pointer() {
    assert_eq!(
        drag_origin(p(100.0, 200.0), p(150.0, 230.0), p(10.0, 900.0)),
        p(-40.0, 870.0)
    );
}

#[test]
fn settled_pill_origin_keeps_dropped_pill() {
    let cases = [
        (anchor(HAnchor::Trailing, false), p(850.0, 918.0)),
        (anchor(HAnchor::Leading, true), p(100.0, 50.0)),
        (anchor(HAnchor::Center, false), p(694.0, 918.0)),
        (anchor(HAnchor::Trailing, true), p(1300.0, 300.0)),
    ];
    for (a, o) in cases {
        let w = canvas_frame(o, a);
        assert_eq!(
            settled_pill_origin(w.origin(), w.origin(), w.origin(), w.size(), a, SCR),
            o
        );
    }
    // The anchor changes at drop (centre of the pill is in the middle third) but the pill does not move.
    assert_eq!(near(pill_centre(p(850.0, 918.0)), SCR).h, HAnchor::Center);
}

#[test]
fn dropped_pill_stays_put_and_card_fits_after_reanchor() {
    let o = p(850.0, 918.0);
    let a = near(pill_centre(o), SCR);
    let canvas = canvas_frame(o, a);
    assert_eq!(pill(canvas, COLLAPSED, a).origin(), o);
    let card = pill(canvas, sz(EXPANDED_WIDTH, 400.0), a);
    assert!(card.min_x() >= SCR.min_x() && card.max_x() <= SCR.max_x());
}

/// Drag the canvas of a pill at `o` by `(dx, dy)`, clamp it as the app does and settle: `(settled, shown)`.
fn drop_pill(o: Point, a: Anchor, dx: f64, dy: f64, height: f64) -> (Point, Point) {
    let w0 = canvas_frame(o, a);
    let raw = p(w0.x + dx, w0.y + dy);
    let clamped = clamp_drag(raw, w0.size(), sz(EXPANDED_WIDTH, height), a, SCR);
    let w = Rect::from_origin_size(clamped, w0.size());
    (
        settled_pill_origin(w0.origin(), raw, clamped, w0.size(), a, SCR),
        pill(w, COLLAPSED, a).origin(),
    )
}

#[test]
fn dropped_far_from_the_pill_still_reaches_the_left_edge() {
    let o = p(1386.0, 950.0);
    let a = near(pill_centre(o), SCR);
    // Grabbed far left of the pill: the pill ends 4 pt short of where the card still fits, so the card is pushed against the edge.
    let fits = COLLAPSED.w - EXPANDED_WIDTH;
    let (settled, _) = drop_pill(o, a, -(o.x + fits + 4.0), 0.0, 400.0);
    assert_eq!(settled.x, 0.0);
    assert_eq!(near(pill_centre(settled), SCR).h, HAnchor::Leading);
}

#[test]
fn overshooting_on_the_pill_reaches_the_left_edge() {
    let o = p(1386.0, 950.0);
    let a = near(pill_centre(o), SCR);
    assert_eq!(drop_pill(o, a, -1400.0, 0.0, 400.0).0.x, 0.0);
}

#[test]
fn dropped_past_the_bottom_reaches_the_bottom_edge() {
    let o = p(1300.0, 918.0);
    let a = anchor(HAnchor::Trailing, false);
    let (settled, _) = drop_pill(o, a, 0.0, -1200.0, 550.0);
    assert_eq!(settled.y, 0.0);
    assert!(near(pill_centre(settled), SCR).up);
}

#[test]
fn drop_inside_the_screen_keeps_the_pill_where_shown() {
    let o = p(850.0, 600.0);
    let a = near(pill_centre(o), SCR);
    let (settled, shown) = drop_pill(o, a, -100.0, -50.0, 400.0);
    assert_eq!(settled, shown);
}

#[test]
fn horizontal_drag_of_an_overflowing_card_does_not_snap_vertically() {
    let o = p(1300.0, 480.0);
    let a = anchor(HAnchor::Trailing, false);
    assert_eq!(near(pill_centre(o), SCR), a);
    let canvas = canvas_frame(o, a);
    let raw = p(canvas.x - 10.0, canvas.y);
    let clamped = clamp_drag(raw, canvas.size(), sz(EXPANDED_WIDTH, 600.0), a, SCR);
    let shown = pill(Rect::from_origin_size(clamped, canvas.size()), COLLAPSED, a).origin();
    let settled = settled_pill_origin(canvas.origin(), raw, clamped, canvas.size(), a, SCR);
    assert_eq!(settled.y, shown.y);
    assert_ne!(settled.y, 0.0);
}

#[test]
fn fractional_drop_inside_the_screen_keeps_the_pill() {
    let o = p(850.0, 600.0);
    let a = near(pill_centre(o), SCR);
    let w0 = canvas_frame(o, a);
    let raw = p(w0.x - 100.4, w0.y - 50.3);
    let clamped = clamp_drag(raw, w0.size(), sz(EXPANDED_WIDTH, 400.0), a, SCR);
    let shown = pill(Rect::from_origin_size(clamped, w0.size()), COLLAPSED, a).origin();
    assert!((shown.x - 749.6).abs() < 0.001 && (shown.y - 549.7).abs() < 0.001);
    assert_eq!(
        settled_pill_origin(w0.origin(), raw, clamped, w0.size(), a, SCR),
        shown
    );
    // A frame read back after AppKit's rounding (built by hand here) would not give the shown pill.
    let rounded = p(clamped.x.floor(), clamped.y.floor());
    assert_ne!(
        settled_pill_origin(w0.origin(), raw, rounded, w0.size(), a, SCR),
        shown
    );
}

#[test]
fn clamp_drag_stops_visible_card_at_edges() {
    let size = CANVAS;
    let down = anchor(HAnchor::Trailing, false);
    let up = anchor(HAnchor::Trailing, true);
    let pill_rect = |o: Point, v: Size, a: Anchor| pill(Rect::from_origin_size(o, size), v, a);
    let c = COLLAPSED;
    let inside = p(600.0, 300.0);
    // Past max_x: pill max_x is 50 beyond the screen edge.
    let right = p(SCR.max_x() + 50.0 + MARGIN - size.w, inside.y);
    assert_eq!(
        pill_rect(clamp_drag(right, size, c, down, SCR), c, down).max_x(),
        SCR.max_x()
    );
    let left = p(-50.0 - (size.w - MARGIN - c.w), inside.y);
    assert_eq!(
        pill_rect(clamp_drag(left, size, c, down, SCR), c, down).min_x(),
        SCR.min_x()
    );
    let top = p(inside.x, SCR.max_y() + 50.0 + MARGIN - size.h);
    assert_eq!(
        pill_rect(clamp_drag(top, size, c, down, SCR), c, down).max_y(),
        SCR.max_y()
    );
    let bottom = p(inside.x, -50.0 - MARGIN);
    assert_eq!(
        pill_rect(clamp_drag(bottom, size, c, up, SCR), c, up).min_y(),
        SCR.min_y()
    );
    let card = sz(EXPANDED_WIDTH, 400.0);
    assert_eq!(
        pill_rect(clamp_drag(right, size, card, down, SCR), card, down).max_x(),
        SCR.max_x()
    );
    assert_eq!(clamp_drag(inside, size, c, down, SCR), inside);
}

#[test]
fn relative_origin_keeps_edge_distances() {
    let t = anchor(HAnchor::Trailing, false);
    assert_eq!(
        relative_origin(
            p(SCR.max_x() - 126.0 - 16.0, SCR.max_y() - 32.0 - 10.0),
            SCR,
            EXT,
            t
        ),
        p(EXT.max_x() - 142.0, EXT.max_y() - 42.0)
    );
    let l = anchor(HAnchor::Leading, true);
    assert_eq!(
        relative_origin(p(SCR.min_x() + 20.0, SCR.min_y() + 30.0), SCR, EXT, l),
        p(EXT.min_x() + 20.0, EXT.min_y() + 30.0)
    );
    let c = anchor(HAnchor::Center, false);
    assert_eq!(
        relative_origin(p(SCR.mid_x() + 40.0, SCR.max_y() - 50.0), SCR, EXT, c),
        p(EXT.mid_x() + 40.0, EXT.max_y() - 50.0)
    );
}

#[test]
fn relative_origin_clamps_to_target() {
    let o = p(EXT.max_x() - 2000.0, 100.0);
    let r = relative_origin(o, EXT, SCR, anchor(HAnchor::Trailing, true));
    assert_eq!(r, clamp(r, SCR));
    assert!(r.x >= SCR.min_x() && r.x <= SCR.max_x() - COLLAPSED.w);
    assert!(r.y >= SCR.min_y() && r.y <= SCR.max_y() - COLLAPSED.h);
}

#[test]
fn legacy_offset_picks_screen_containing_centre() {
    let r = legacy_offset(1512.0 + 300.0, 200.0, COLLAPSED.w, &[SCR, EXT]).unwrap();
    assert_eq!(r.0, 1);
    assert_eq!(r.1, [300.0, 200.0]);
}

#[test]
fn legacy_offset_falls_back_to_first_screen_and_clamps() {
    let r = legacy_offset(9000.0, 9000.0, COLLAPSED.w, &[SCR, EXT]).unwrap();
    assert_eq!(r.0, 0);
    assert_eq!(r.1, [SCR.max_x() - COLLAPSED.w, SCR.max_y() - COLLAPSED.h]);
    assert!(legacy_offset(0.0, 0.0, 104.0, &[]).is_none());
}

#[test]
fn legacy_offset_shifts_legacy_width() {
    let x = SCR.max_x() - 232.0 - 14.0;
    let r = legacy_offset(x, 800.0, 232.0, &[SCR]).unwrap();
    assert_eq!(r.1, [x + 232.0 - COLLAPSED.w, 800.0]);
}

#[test]
fn offset_round_trips_across_origin_change() {
    let o = p(1512.0 + 300.0, 900.0);
    let off = offset(o, EXT);
    assert_eq!(off, [300.0, 900.0]);
    assert_eq!(
        origin(&off, Rect::new(0.0, 0.0, 2560.0, 1440.0)),
        Some(p(300.0, 900.0))
    );
    assert_eq!(origin(&[1.0], EXT), None);
}
