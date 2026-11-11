use claude_sessions_macos::hid;

fn nums<const N: usize>(args: &[String]) -> Result<[f64; N], String> {
    if args.len() != N {
        return Err(format!("expected {N} numbers"));
    }
    let mut out = [0.0; N];
    for (slot, arg) in out.iter_mut().zip(args) {
        *slot = arg.parse().map_err(|_| format!("not a number: {arg}"))?;
    }
    Ok(out)
}

pub fn run(args: &[String]) -> Result<(), String> {
    if !hid::trusted() {
        return Err("Accessibility is not granted to this terminal (System Settings > Privacy & Security > Accessibility)".into());
    }
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("move") => {
            let [x, y] = nums(rest)?;
            hid::move_to(x, y);
        }
        Some("click") => {
            let [x, y] = nums(rest)?;
            hid::click(x, y);
        }
        Some("rightclick") => {
            let [x, y] = nums(rest)?;
            hid::right_click(x, y);
        }
        Some("drag") => {
            let [x1, y1, x2, y2] = nums(rest)?;
            hid::drag(x1, y1, x2, y2);
        }
        Some("keys") => hid::keys(rest.first().ok_or("missing combo")?)?,
        _ => return Err("hid: move | click | rightclick | drag | keys".into()),
    }
    Ok(())
}
