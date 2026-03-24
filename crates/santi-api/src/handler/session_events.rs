use axum::response::sse::Event;

use crate::schema::session_events::{
    SessionCompletedEvent, SessionOutputTextDeltaEvent, SessionStreamEvent,
};

pub fn encode_session_sse_event(event: SessionStreamEvent) -> Event {
    let data = match event {
        SessionStreamEvent::OutputTextDelta(text) => {
            serde_json::to_string(&SessionOutputTextDeltaEvent {
                event_type: "response.output_text.delta",
                delta: text,
            })
            .unwrap_or_else(|_| "{}".to_string())
        }
        SessionStreamEvent::Completed => serde_json::to_string(&SessionCompletedEvent {
            event_type: "response.completed",
        })
        .unwrap_or_else(|_| "{}".to_string()),
    };

    Event::default().data(data)
}

pub fn done_event() -> Event {
    Event::default().data("[DONE]")
}
