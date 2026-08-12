use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;

static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InteractionEvent {
    pub sequence: u64,
    pub kind: String,
    pub value: String,
    pub intensity: f32,
    pub source: String,
    pub generated_at: String,
}

pub fn channel(capacity: usize) -> broadcast::Sender<InteractionEvent> {
    let (sender, _) = broadcast::channel(capacity.max(16));
    sender
}

pub fn publish(
    sender: &broadcast::Sender<InteractionEvent>,
    kind: &str,
    value: &str,
    intensity: f32,
    source: &str,
) -> InteractionEvent {
    let event = InteractionEvent {
        sequence: EVENT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        kind: kind.to_string(),
        value: value.to_string(),
        intensity: intensity.clamp(0.0, 1.0),
        source: source.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
    };
    let _ = sender.send(event.clone());
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn published_events_are_bounded_and_received() {
        let sender = channel(16);
        let mut receiver = sender.subscribe();
        let event = publish(&sender, "expression", "happy", 2.0, "test");
        let received = receiver.recv().await.unwrap();
        assert_eq!(received, event);
        assert_eq!(received.intensity, 1.0);
    }
}
