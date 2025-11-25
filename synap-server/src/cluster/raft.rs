//! Raft Consensus Algorithm (Simplified)
//!
//! Implements a simplified Raft consensus for cluster coordination.
//! Used for leader election and cluster state coordination.

use super::types::ClusterResult;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info};

/// Raft node state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaftState {
    /// Follower state
    Follower,
    /// Candidate (during election)
    Candidate,
    /// Leader state
    Leader,
}

/// Raft log entry
#[derive(Debug, Clone)]
pub struct RaftLogEntry {
    pub term: u64,
    pub index: u64,
    pub command: Vec<u8>,
}

/// Raft node
pub struct RaftNode {
    /// Node ID
    #[allow(dead_code)]
    node_id: String,

    /// Current state
    state: Arc<RwLock<RaftState>>,

    /// Current term
    #[allow(dead_code)]
    current_term: Arc<RwLock<u64>>,

    /// Voted for (node ID) in current term
    voted_for: Arc<RwLock<Option<String>>>,

    /// Log entries
    #[allow(dead_code)]
    log: Arc<RwLock<Vec<RaftLogEntry>>>,

    /// Commit index
    #[allow(dead_code)]
    commit_index: Arc<RwLock<u64>>,

    /// Last applied index
    #[allow(dead_code)]
    last_applied: Arc<RwLock<u64>>,

    /// Election timeout
    #[allow(dead_code)]
    election_timeout: Duration,

    /// Heartbeat interval
    #[allow(dead_code)]
    heartbeat_interval: Duration,

    /// Last heartbeat received
    last_heartbeat: Arc<RwLock<u64>>,

    /// Channel for Raft commands
    #[allow(dead_code)]
    raft_tx: mpsc::UnboundedSender<RaftCommand>,
}

#[allow(dead_code)]
enum RaftCommand {
    VoteRequest {
        candidate_id: String,
        term: u64,
        last_log_index: u64,
        last_log_term: u64,
    },
    VoteResponse {
        voter_id: String,
        term: u64,
        vote_granted: bool,
    },
    AppendEntries {
        leader_id: String,
        term: u64,
        prev_log_index: u64,
        prev_log_term: u64,
        entries: Vec<RaftLogEntry>,
        leader_commit: u64,
    },
    Heartbeat {
        leader_id: String,
        term: u64,
    },
}

impl RaftNode {
    /// Create new Raft node
    pub fn new(node_id: String, election_timeout: Duration, heartbeat_interval: Duration) -> Self {
        let (raft_tx, raft_rx) = mpsc::unbounded_channel();

        let state = Arc::new(RwLock::new(RaftState::Follower));
        let current_term = Arc::new(RwLock::new(0));
        let voted_for = Arc::new(RwLock::new(None));
        let log = Arc::new(RwLock::new(Vec::new()));
        let commit_index = Arc::new(RwLock::new(0));
        let last_applied = Arc::new(RwLock::new(0));
        let last_heartbeat = Arc::new(RwLock::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        ));

        let node_id_clone = node_id.clone();
        let state_clone = Arc::clone(&state);
        let current_term_clone = Arc::clone(&current_term);
        let voted_for_clone = Arc::clone(&voted_for);
        let last_heartbeat_clone = Arc::clone(&last_heartbeat);

        // Spawn Raft worker
        tokio::spawn(Self::raft_worker(
            node_id_clone,
            state_clone,
            current_term_clone,
            voted_for_clone,
            last_heartbeat_clone,
            election_timeout,
            heartbeat_interval,
            raft_rx,
        ));

        Self {
            node_id,
            state,
            current_term,
            voted_for,
            log,
            commit_index,
            last_applied,
            election_timeout,
            heartbeat_interval,
            last_heartbeat,
            raft_tx,
        }
    }

    /// Get current state
    pub fn state(&self) -> RaftState {
        *self.state.read()
    }

    /// Get current term
    pub fn current_term(&self) -> u64 {
        *self.current_term.read()
    }

    /// Check if this node is leader
    pub fn is_leader(&self) -> bool {
        self.state() == RaftState::Leader
    }

    /// Request vote (for candidate)
    pub fn request_vote(&self, candidate_id: &str, term: u64) -> ClusterResult<bool> {
        let mut current_term = self.current_term.write();
        let mut voted_for = self.voted_for.write();

        // If term is outdated, reject
        if term < *current_term {
            return Ok(false);
        }

        // If term is newer, update term and reset vote
        if term > *current_term {
            *current_term = term;
            *voted_for = None;
        }

        // Vote if haven't voted or voted for same candidate
        if voted_for.is_none() || voted_for.as_ref() == Some(&candidate_id.to_string()) {
            *voted_for = Some(candidate_id.to_string());
            info!("Voted for {} in term {}", candidate_id, term);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Receive heartbeat from leader
    pub fn receive_heartbeat(&self, leader_id: &str, term: u64) -> ClusterResult<()> {
        let mut current_term = self.current_term.write();
        let mut state = self.state.write();
        let mut last_heartbeat = self.last_heartbeat.write();

        // Update term if newer
        if term >= *current_term {
            *current_term = term;
            *state = RaftState::Follower;
            *last_heartbeat = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            debug!("Received heartbeat from {} in term {}", leader_id, term);
        }

        Ok(())
    }

    /// Raft worker (handles elections and heartbeats)
    #[allow(clippy::too_many_arguments)]
    async fn raft_worker(
        node_id: String,
        state: Arc<RwLock<RaftState>>,
        current_term: Arc<RwLock<u64>>,
        voted_for: Arc<RwLock<Option<String>>>,
        last_heartbeat: Arc<RwLock<u64>>,
        election_timeout: Duration,
        heartbeat_interval: Duration,
        mut raft_rx: mpsc::UnboundedReceiver<RaftCommand>,
    ) {
        let mut election_timer = interval(election_timeout);
        let mut heartbeat_timer = interval(heartbeat_interval);

        loop {
            tokio::select! {
                _ = election_timer.tick() => {
                    // Check if we should start election
                    let state_val = *state.read();
                    let last_hb = *last_heartbeat.read();
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    // If follower and haven't received heartbeat, become candidate
                    if state_val == RaftState::Follower && (now - last_hb) > election_timeout.as_secs() {
                        info!("Starting election: node {} term {}", node_id, *current_term.read() + 1);

                        let mut term = current_term.write();
                        *term += 1;
                        *state.write() = RaftState::Candidate;
                        *voted_for.write() = Some(node_id.clone());

                        // TODO: Request votes from other nodes
                        // For now, if single node, become leader immediately
                        *state.write() = RaftState::Leader;
                        info!("Elected as leader: node {} term {}", node_id, *term);
                    }
                }
                _ = heartbeat_timer.tick() => {
                    // If leader, send heartbeats
                    if *state.read() == RaftState::Leader {
                        debug!("Sending heartbeat as leader: node {}", node_id);
                        // TODO: Send heartbeats to followers
                    }
                }
                Some(cmd) = raft_rx.recv() => {
                    match cmd {
                        RaftCommand::VoteRequest { candidate_id, term, .. } => {
                            // Handle vote request
                            debug!("Received vote request from {} term {}", candidate_id, term);
                        }
                        RaftCommand::VoteResponse { voter_id, term, vote_granted } => {
                            // Handle vote response
                            debug!("Received vote response from {} term {} granted: {}", voter_id, term, vote_granted);
                        }
                        RaftCommand::AppendEntries { leader_id, term, .. } => {
                            // Handle append entries
                            debug!("Received append entries from {} term {}", leader_id, term);
                        }
                        RaftCommand::Heartbeat { leader_id: _, term } => {
                            // Handle heartbeat
                            let mut current_term = current_term.write();
                            let mut state = state.write();
                            let mut last_heartbeat = last_heartbeat.write();

                            if term >= *current_term {
                                *current_term = term;
                                *state = RaftState::Follower;
                                *last_heartbeat = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                            }
                        }
                    }
                }
            }
        }
    }
}
