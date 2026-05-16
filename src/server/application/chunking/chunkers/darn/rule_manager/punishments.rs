pub type PunishmentFn = fn(usize, usize, &mut [usize]);

pub fn const_punishment(scale: usize, length: usize, out: &mut [usize]) {
    for slot in out.iter_mut().take(length) {
        *slot = scale;
    }
}

pub fn reverse_linear_punishment(_scale: usize, length: usize, out: &mut [usize]) {
    let start = 50usize;
    for (i, slot) in out.iter_mut().enumerate().take(length) {
        *slot = start.saturating_sub(i);
    }
}

pub fn inverse_triangular_punishment(scale: usize, length: usize, out: &mut [usize]) {
    if length == 0 {
        return;
    }
    if length == 1 {
        out[0] = 0;
        return;
    }

    let mid = (length - 1) as f64 / 2.0;
    let max_distance = mid;

    for (i, val) in out.iter_mut().enumerate().take(length) {
        let distance_from_mid = (i as f64 - mid).abs();
        let normalized = distance_from_mid / max_distance;
        *val = (normalized * scale as f64).round() as usize;
    }
}
