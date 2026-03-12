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
    extract::State,
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
#[derive(Clone)]
struct AppState {
    /// Maps event_id to (room_id, accumulated_content)
    pending_edits: Arc<Mutex<HashMap<OwnedEventId, (OwnedRoomId, String)>>>,
    /// Matrix client for editing messages
    client: Arc<Mutex<Option<Client>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            pending_edits: Arc::new(Mutex::new(HashMap::new())),
            client: Arc::new(Mutex::new(None)),
        }
    }
}

/// The messages to stream via SSE
const SSE_MESSAGES: &[&str] = &[
    "Breaking: New Makepad release brings revolutionary UI performance!",
    "Tech Update: Rust continues to dominate systems programming",
    "Local News: Community embraces new SSE widget for real-time updates",
    "Science: Researchers achieve breakthrough in quantum computing",
    "Weather: Sunny skies expected throughout the week",
];

/// SSE handler that streams messages and edits Matrix message when complete
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Collect all content for the final edit
    let all_content: String = SSE_MESSAGES
        .iter()
        .map(|s| *s)
        .collect::<Vec<_>>()
        .join("\n");

    // Create a channel to signal stream completion
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let state_clone = state.clone();
    let tx_clone = tx.clone();

    let stream = stream::iter(SSE_MESSAGES.iter().enumerate())
        .chain(stream::once(async { (SSE_MESSAGES.len(), &"[STREAM_END]") }))
        .throttle(Duration::from_secs(1))
        .map(move |(i, msg)| {
            let is_end = *msg == "[STREAM_END]";
            let data = if is_end {
                format!(r#"{{"id": {}, "status": "complete"}}"#, i)
            } else {
                format!(
                    r#"{{"id": {}, "title": "News #{}", "content": "{}"}}"#,
                    i,
                    i + 1,
                    msg
                )
            };

            // Signal completion when STREAM_END is sent
            if is_end {
                let tx = tx_clone.clone();
                tokio::spawn(async move {
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(());
                    }
                });
            }

            Ok(Event::default().data(data))
        });

    // Spawn a task that waits for stream completion signal, then edits the message
    let content_for_edit = all_content;
    tokio::spawn(async move {
        // Wait for the stream to signal completion
        let _ = rx.await;
        // Edit the pending Matrix message
        edit_pending_message(state_clone, content_for_edit).await;
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

/// Edit the most recent pending Matrix message with the accumulated content
async fn edit_pending_message(state: AppState, content: String) {
    let client_guard = state.client.lock().await;
    let Some(client) = client_guard.as_ref() else {
        eprintln!("Matrix bot: No client available for editing");
        return;
    };

    // Get the first pending edit (there should be one per SSE request)
    let pending_edit = {
        let mut pending = state.pending_edits.lock().await;
        pending.drain().next()
    };

    if let Some((event_id, (room_id, _))) = pending_edit {
        if let Some(room) = client.get_room(&room_id) {
            let final_content = format!("SSE Stream Complete:\n\n{}", content);

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
        }
    } else {
        println!("Matrix bot: No pending message to edit");
    }
}

/// Run the Matrix bot that responds to /sse commands
async fn run_matrix_bot(state: AppState) -> anyhow::Result<()> {
    // let homeserver_url = "http://localhost:8008";
    // let username = "testuser2";
    // let password = "testpassword";
    let homeserver_url = "https://matrix.org";
    let username = "ruitobeta";
    let password = "!C6WS3hcPGM:GMa";
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

    // Store the client in shared state for later use
    {
        let mut client_guard = state.client.lock().await;
        *client_guard = Some(client.clone());
    }

    // Clone state for the event handler
    let state_clone = state.clone();

    // Add event handler for room messages
    client.add_event_handler(
        move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let state = state_clone.clone();
            async move {
                handle_room_message(event, room, state).await;
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
    state: AppState,
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

                // Store the event_id for later editing (will be edited when SSE stream ends)
                {
                    let mut pending = state.pending_edits.lock().await;
                    pending.insert(event_id, (room_id, String::new()));
                }
            }
            Err(e) => {
                eprintln!("Matrix bot: Failed to send message: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Starting SSE Server with Matrix Bot...");

    // Create shared state
    let state = AppState::default();

    // Create SSE server with state
    let app = Router::new()
        .route("/events", get(sse_handler))
        .with_state(state.clone());

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
