//! SSE Server with Matrix Bot
//!
//! This binary runs:
//! 1. An SSE (Server-Sent Events) server at http://127.0.0.1:3001/events
//! 2. A Matrix bot (testuser2) that responds to "/sse" commands with SSE stream headers
//!
//! When the bot receives "/sse", it sends a message with format:
//! `!SSE|http://127.0.0.1:3001/events|`
//!
//! After streaming completes, the bot edits the message with the full content.

#![recursion_limit = "256"]

use axum::{
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::{self, Stream};
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use matrix_sdk::{
    Client,
    config::SyncSettings,
    room::Room,
    ruma::{
        OwnedEventId, OwnedRoomId,
        events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
            ReplacementMetadata,
        },
    },
};

/// Shared state for tracking SSE messages that need to be edited
#[derive(Clone, Default)]
struct SseState {
    /// Maps event_id to (room_id, accumulated_content)
    pending_edits: Arc<Mutex<HashMap<OwnedEventId, (OwnedRoomId, String)>>>,
}

/// The messages to stream via SSE
const SSE_MESSAGES: &[&str] = &[
    "Breaking: New Makepad release brings revolutionary UI performance!",
    "Tech Update: Rust continues to dominate systems programming",
    "Local News: Community embraces new SSE widget for real-time updates",
    "Science: Researchers achieve breakthrough in quantum computing",
    "Weather: Sunny skies expected throughout the week",
    "[STREAM_END]", // Marker for end of stream
];

/// SSE handler that streams messages and signals completion
async fn sse_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::iter(SSE_MESSAGES.iter().enumerate())
        .throttle(Duration::from_secs(1))
        .map(|(i, msg)| {
            let data = if *msg == "[STREAM_END]" {
                format!(r#"{{"id": {}, "status": "complete"}}"#, i)
            } else {
                format!(
                    r#"{{"id": {}, "title": "News #{}", "content": "{}"}}"#,
                    i,
                    i + 1,
                    msg
                )
            };
            Ok(Event::default().data(data))
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

/// Run the Matrix bot that responds to /sse commands
async fn run_matrix_bot(state: SseState) -> anyhow::Result<()> {
    let homeserver_url = "http://localhost:8008";
    let username = "testuser2";
    let password = "testpassword";

    println!("Matrix bot: Connecting to homeserver {}", homeserver_url);

    // Build the client
    let client = Client::builder()
        .homeserver_url(homeserver_url)
        .build()
        .await?;

    // Login
    client
        .matrix_auth()
        .login_username(username, password)
        .initial_device_display_name("sse-bot")
        .send()
        .await?;

    println!("Matrix bot: Logged in as {}", username);

    // Clone state for the event handler
    let state_clone = state.clone();
    let client_clone = client.clone();

    // Add event handler for room messages
    client.add_event_handler(
        move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let state = state_clone.clone();
            let client = client_clone.clone();
            async move {
                handle_room_message(event, room, state, client).await;
            }
        },
    );

    // Start syncing
    println!("Matrix bot: Starting sync...");
    client.sync(SyncSettings::default()).await?;

    Ok(())
}

/// Handle incoming room messages
async fn handle_room_message(
    event: OriginalSyncRoomMessageEvent,
    room: Room,
    state: SseState,
    client: Client,
) {
    // Only handle text messages
    let MessageType::Text(text_content) = &event.content.msgtype else {
        return;
    };

    let body = text_content.body.trim();

    // Check if message is "/sse" command
    if body == "/sse" {
        println!("Matrix bot: Received /sse command from {}", event.sender);

        // Send SSE header message
        let sse_message = "!SSE|http://127.0.0.1:3001/events|";
        let content = RoomMessageEventContent::text_plain(sse_message);

        match room.send(content).await {
            Ok(response) => {
                let event_id = response.response.event_id;
                let room_id = room.room_id().to_owned();

                println!(
                    "Matrix bot: Sent SSE header message, event_id: {}",
                    event_id
                );

                // Store the event_id for later editing
                {
                    let mut pending = state.pending_edits.lock().await;
                    pending.insert(event_id.clone(), (room_id, String::new()));
                }

                // Spawn a task to fetch SSE and accumulate content
                let state_clone = state.clone();
                let client_clone = client.clone();
                tokio::spawn(async move {
                    fetch_sse_and_edit(event_id, state_clone, client_clone).await;
                });
            }
            Err(e) => {
                eprintln!("Matrix bot: Failed to send message: {}", e);
            }
        }
    }
}

/// Fetch SSE content and edit the original message when complete
async fn fetch_sse_and_edit(event_id: OwnedEventId, state: SseState, client: Client) {
    println!("Matrix bot: Starting SSE fetch for event {}", event_id);

    let sse_url = "http://127.0.0.1:3001/events";

    // Use reqwest to fetch SSE
    let response = match reqwest::Client::new()
        .get(sse_url)
        .header("Accept", "text/event-stream")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Matrix bot: Failed to connect to SSE server: {}", e);
            return;
        }
    };

    let mut accumulated_content = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                let text = String::from_utf8_lossy(&chunk);
                // Parse SSE data lines
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        // Parse JSON to extract content
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                                if !accumulated_content.is_empty() {
                                    accumulated_content.push_str("\n");
                                }
                                accumulated_content.push_str(content);
                            }
                            // Check for stream end
                            if json.get("status").and_then(|s| s.as_str()) == Some("complete") {
                                println!("Matrix bot: SSE stream completed");
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Matrix bot: Error reading SSE stream: {}", e);
                break;
            }
        }
    }

    // Now edit the original message with accumulated content
    let room_id = {
        let pending = state.pending_edits.lock().await;
        pending.get(&event_id).map(|(room_id, _)| room_id.clone())
    };

    if let Some(room_id) = room_id {
        if let Some(room) = client.get_room(&room_id) {
            let final_content = format!(
                "SSE Stream Complete:\n\n{}",
                accumulated_content
            );

            // Create edit content using ReplacementMetadata
            let metadata = ReplacementMetadata::new(event_id.clone(), None);
            let new_content = RoomMessageEventContent::text_plain(&final_content)
                .make_replacement(metadata);

            match room.send(new_content).await {
                Ok(_) => {
                    println!("Matrix bot: Successfully edited message {}", event_id);
                }
                Err(e) => {
                    eprintln!("Matrix bot: Failed to edit message: {}", e);
                }
            }

            // Remove from pending edits
            let mut pending = state.pending_edits.lock().await;
            pending.remove(&event_id);
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Starting SSE Server with Matrix Bot...");

    // Create shared state
    let state = SseState::default();

    // Create SSE server
    let app = Router::new().route("/events", get(sse_handler));

    // Spawn SSE server
    let sse_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
            .await
            .unwrap();
        println!("SSE Server running on http://127.0.0.1:3001/events");
        axum::serve(listener, app).await.unwrap();
    });

    // Give SSE server a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Run Matrix bot
    let bot_handle = tokio::spawn(async move {
        if let Err(e) = run_matrix_bot(state).await {
            eprintln!("Matrix bot error: {}", e);
        }
    });

    // Wait for both to complete (they won't under normal operation)
    tokio::select! {
        _ = sse_handle => println!("SSE server stopped"),
        _ = bot_handle => println!("Matrix bot stopped"),
    }
}
