use std::{io::ErrorKind, sync::Arc};
use makepad_widgets::*;
use matrix_sdk::{Client, RoomState, ruma::{OwnedRoomId, UserId}};
use rustc_hash::{FxBuildHasher, FxHashSet};
use strpack::{Reader, Writer};
use crate::{home::rooms_list::{SavedSpace, SavedSpaceGraph, SpaceChildren}, persistence::user_cache_dir, space_service_sync::ParentChain};

const SPACE_GRAPH_FILE_NAME: &str = "space_graph.bin";

/// We bump this first byte of the file whenever its layout changes. Then comes the count of spaces,
/// and each space has its ID, then its parent chain, rooms and subspaces as counted lists of IDs.
const SPACE_GRAPH_FILE_VERSION: u8 = 1;

/// Saves the given user's space graph to the cache dir.
pub fn save_space_graph(space_graph: &SavedSpaceGraph, user_id: &UserId) -> anyhow::Result<()> {
    let num_ids: usize = space_graph.values()
        .map(|saved_space| 1 + saved_space.parent_chain.len() + saved_space.children.rooms.len() + saved_space.children.subspaces.len())
        .sum();
    // Room IDs are almost always shorter than 48 bytes, so this rarely has to grow.
    let mut writer = Writer::with_capacity(5 + 12 * space_graph.len() + 48 * num_ids);
    writer.write_u8(SPACE_GRAPH_FILE_VERSION);
    writer.write_count(space_graph.len());
    for (space_id, saved_space) in space_graph {
        writer.write_str(space_id.as_str());
        writer.write_count(saved_space.parent_chain.len());
        saved_space.parent_chain.iter().for_each(|id| writer.write_str(id.as_str()));
        for ids in [&saved_space.children.rooms, &saved_space.children.subspaces] {
            writer.write_count(ids.len());
            ids.iter().for_each(|id| writer.write_str(id.as_str()));
        }
    }
    let bytes = writer.into_bytes();
    let dir = user_cache_dir(user_id);
    let path = dir.join(SPACE_GRAPH_FILE_NAME);
    if let Err(e) = std::fs::write(&path, &bytes) {
        // The OS can clear out the cache dir at any time, so we might need to create it again.
        if e.kind() != ErrorKind::NotFound { return Err(e.into()) }
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, &bytes)?;
    }
    log!("Successfully saved the space graph to the cache dir.");
    Ok(())
}

/// Deletes the given user's space graph from the cache dir, if there is one.
pub async fn delete_space_graph(user_id: &UserId) {
    let path = user_cache_dir(user_id).join(SPACE_GRAPH_FILE_NAME);
    if let Err(e) = tokio::fs::remove_file(&path).await && e.kind() != ErrorKind::NotFound {
        error!("Failed to delete the space graph at {}: {e}", path.display());
    }
}

/// Loads the given user's space graph from the cache dir, without the spaces they've left.
///
/// Returns an empty graph if there's no saved graph, and stops at the first part that isn't valid.
pub async fn load_space_graph(client: &Client, user_id: &UserId) -> SavedSpaceGraph {
    let path = user_cache_dir(user_id).join(SPACE_GRAPH_FILE_NAME);
    let (client, user_id) = (client.clone(), user_id.to_owned());
    // Decoding a big graph takes a while, so we do it on the blocking thread along with the read.
    tokio::task::spawn_blocking(move || {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return SavedSpaceGraph::default(),
            Err(e) => {
                error!("Failed to read the space graph from {}: {e}", path.display());
                return SavedSpaceGraph::default();
            }
        };
        let mut reader = Reader::new(&bytes);
        // Every ID takes at least 2 bytes and every space at least 14,
        // so a corrupted count can't claim more IDs or spaces than the file could hold.
        let Some(num_spaces) = reader.read_u8()
            .filter(|&version| version == SPACE_GRAPH_FILE_VERSION)
            .and_then(|_| reader.read_count(14))
        else {
            return SavedSpaceGraph::default();
        };
        let read_id = |reader: &mut Reader| reader.read_str().and_then(|id| OwnedRoomId::try_from(id).ok());
        let read_id_set = |reader: &mut Reader| -> Option<FxHashSet<OwnedRoomId>> {
            let count = reader.read_count(2)?;
            let mut ids = FxHashSet::with_capacity_and_hasher(count, FxBuildHasher);
            for _ in 0..count {
                ids.insert(read_id(reader)?);
            }
            Some(ids)
        };
        let mut space_graph = SavedSpaceGraph::with_capacity_and_hasher(num_spaces, FxBuildHasher);
        for _ in 0..num_spaces {
            let Some(space_id) = read_id(&mut reader) else { break };
            let Some(chain_len) = reader.read_count(2) else { break };
            let Some(parent_chain) = (0..chain_len).map(|_| read_id(&mut reader)).collect::<Option<ParentChain>>() else { break };
            let Some(rooms) = read_id_set(&mut reader) else { break };
            let Some(subspaces) = read_id_set(&mut reader) else { break };
            space_graph.insert(space_id, SavedSpace {
                parent_chain,
                children: SpaceChildren { rooms: Arc::new(rooms), subspaces: Arc::new(subspaces) },
            });
        }
        if space_graph.len() < num_spaces || !reader.is_empty() {
            error!("Failed to decode part of the space graph in {}.", path.display());
        }
        // We drop the spaces we've left, unless we're still in one of the spaces they're within.
        // Saving can't do this, since it only knows about the spaces we've fetched this session.
        let mut dropped_spaces: SavedSpaceGraph = space_graph.extract_if(|space_id, saved_space| {
            let mut is_left = false;
            for id in std::iter::once(space_id).chain(&saved_space.parent_chain) {
                match client.get_room(id).map(|room| room.state()) {
                    Some(RoomState::Joined) => return false,
                    Some(RoomState::Left | RoomState::Banned) => is_left = true,
                    _ => { }
                }
            }
            is_left
        }).collect();
        if dropped_spaces.is_empty() { return space_graph }
        // A parent chain is only one path to a space, so we keep a space if one we kept still has it as a subspace.
        let mut to_visit: Vec<OwnedRoomId> = space_graph.keys().cloned().collect();
        while let Some(space_id) = to_visit.pop() {
            let Some(subspace_ids) = space_graph.get(&space_id).map(|saved_space| Arc::clone(&saved_space.children.subspaces)) else { continue };
            for subspace_id in subspace_ids.iter() {
                if let Some(saved_space) = dropped_spaces.remove(subspace_id) {
                    space_graph.insert(subspace_id.clone(), saved_space);
                    to_visit.push(subspace_id.clone());
                }
            }
        }
        if !dropped_spaces.is_empty() && let Err(e) = save_space_graph(&space_graph, &user_id) {
            error!("Failed to save the space graph without the spaces we've left. Error: {e}");
        }
        space_graph
    }).await.unwrap_or_default()
}
