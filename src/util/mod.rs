use crate::models::Note;

pub(crate) fn today_yyyymmdd_local() -> String {
    // Use system local timezone (browser runtime).
    let d = js_sys::Date::new_0();
    let y = d.get_full_year();
    let m = d.get_month() + 1;
    let day = d.get_date();
    format!("{:04}{:02}{:02}", y, m, day)
}

pub(crate) fn next_available_daily_note_title_for_date(
    base: &str,
    existing_notes: &[Note],
) -> String {
    let base = base.trim();

    let mut has_base = false;
    let mut max_suffix: u32 = 1;

    for n in existing_notes {
        let t = n.title.trim();
        if t == base {
            has_base = true;
            continue;
        }

        // Match patterns like: YYYYMMDD-2, YYYYMMDD-3, ...
        if let Some(rest) = t.strip_prefix(&format!("{}-", base)) {
            if let Ok(k) = rest.parse::<u32>() {
                if k >= max_suffix {
                    max_suffix = k;
                }
            }
        }
    }

    if !has_base {
        return base.to_string();
    }

    format!("{}-{}", base, max_suffix.saturating_add(1))
}

pub(crate) fn next_available_daily_note_title(existing_notes: &[Note]) -> String {
    next_available_daily_note_title_for_date(&today_yyyymmdd_local(), existing_notes)
}

pub(crate) fn now_ms() -> i64 {
    js_sys::Date::now().round() as i64
}
