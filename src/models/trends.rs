use std::collections::HashMap;

use chrono::Duration;

use crate::domain::ListenEvent;

/// Returns the most played tracks inside the window as (track_index, play_count).
/// The window is counted back from the latest event in the data, not from "now".
pub fn trending(events: &[ListenEvent], window: Duration, limit: usize) -> Vec<(u32, usize)> {
    let Some(latest) = events.iter().map(|e| e.timestamp).max() else {
        return Vec::new();
    };
    let since = latest - window;

    let mut counts: HashMap<u32, usize> = HashMap::new();
    for e in events.iter().filter(|e| e.timestamp >= since) {
        *counts.entry(e.track_index).or_default() += 1;
    }

    let mut ranked: Vec<(u32, usize)> = counts.into_iter().collect();
    // most played first; ties are broken by track_index so the result is stable
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::event;

    #[test]
    fn trending_counts_only_events_inside_window() {
        let events = vec![
            event(1, 0),
            event(3, 10),
            event(2, 2 * 24 * 60 - 10),
            event(2, 2 * 24 * 60),
        ];
        let top = trending(&events, Duration::days(1), 5);
        assert_eq!(top, vec![(2, 2)]);
    }

    #[test]
    fn trending_on_empty_input_is_empty() {
        assert!(trending(&[], Duration::days(1), 5).is_empty());
    }
}
