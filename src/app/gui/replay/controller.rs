use std::time::Instant;

use super::state::ReplayState;

pub fn tick_replay(state: &mut ReplayState) {
    if !state.is_playing || state.samples.is_empty() {
        return;
    }

    let now = Instant::now();
    let Some(last_tick) = state.last_tick_at else {
        state.last_tick_at = Some(now);
        return;
    };

    let elapsed_ms = now.saturating_duration_since(last_tick).as_millis() as f64;
    state.last_tick_at = Some(now);

    if elapsed_ms <= 0.0 {
        return;
    }

    let Some(first) = state.samples.first() else {
        return;
    };
    let Some(last) = state.samples.last() else {
        return;
    };

    let cursor = state.cursor_ms.unwrap_or(first.collected_at_ms);
    let delta = (elapsed_ms * state.speed).round() as i64;
    let next = cursor.saturating_add(delta);

    if next >= last.collected_at_ms {
        state.cursor_ms = Some(last.collected_at_ms);
        state.is_playing = false;
        state.last_tick_at = None;
    } else {
        state.cursor_ms = Some(next.clamp(first.collected_at_ms, last.collected_at_ms));
    }
}
