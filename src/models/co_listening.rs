use std::collections::{HashMap, HashSet};

use petgraph::graph::{NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;

use crate::domain::Session;

#[derive(Clone)]
pub struct CoListeningConfig {
    /// A pair of tracks needs at least this many shared sessions to get an edge.
    pub min_cooccurrence: u32,
    /// Edges with a lower Jaccard similarity are dropped.
    pub min_jaccard: f64,
    /// Only the first N unique tracks of a session are used. Pair count grows
    /// quadratically with session length, so marathon sessions are capped.
    pub max_session_tracks: usize,
}

impl Default for CoListeningConfig {
    fn default() -> Self {
        CoListeningConfig {
            min_cooccurrence: 2,
            min_jaccard: 0.01,
            max_session_tracks: 200,
        }
    }
}

/// Undirected graph: node = track, edge weight = Jaccard similarity.
pub struct CoListeningGraph {
    graph: UnGraph<u32, f64>,
    nodes: HashMap<u32, NodeIndex>,
}

impl CoListeningGraph {
    pub fn build(sessions: &[Session], config: &CoListeningConfig) -> Self {
        // in how many sessions each track appears
        let mut track_sessions: HashMap<u32, u32> = HashMap::new();
        // in how many sessions each pair appears together; key is (smaller, larger)
        let mut pair_counts: HashMap<(u32, u32), u32> = HashMap::new();

        for session in sessions {
            let tracks = unique_tracks(&session.tracks, config.max_session_tracks);

            for &track in &tracks {
                *track_sessions.entry(track).or_default() += 1;
            }
            for (i, &a) in tracks.iter().enumerate() {
                for &b in &tracks[i + 1..] {
                    let key = if a < b { (a, b) } else { (b, a) };
                    *pair_counts.entry(key).or_default() += 1;
                }
            }
        }

        let mut graph: UnGraph<u32, f64> = UnGraph::new_undirected();
        let mut nodes: HashMap<u32, NodeIndex> = HashMap::new();

        // sorted so that the graph layout is the same on every run
        let mut pairs: Vec<((u32, u32), u32)> = pair_counts.into_iter().collect();
        pairs.sort_unstable_by_key(|&(pair, _)| pair);

        for ((a, b), together) in pairs {
            if together < config.min_cooccurrence {
                continue;
            }
            let (Some(&sessions_a), Some(&sessions_b)) =
                (track_sessions.get(&a), track_sessions.get(&b))
            else {
                continue;
            };

            let union = sessions_a + sessions_b - together;
            let jaccard = f64::from(together) / f64::from(union);
            if jaccard < config.min_jaccard {
                continue;
            }

            let node_a = node_for(&mut graph, &mut nodes, a);
            let node_b = node_for(&mut graph, &mut nodes, b);
            graph.add_edge(node_a, node_b, jaccard);
        }

        CoListeningGraph { graph, nodes }
    }

    /// Tracks most often listened together with the given one, as (track_index, similarity).
    pub fn similar(&self, track_index: u32, limit: usize) -> Vec<(u32, f64)> {
        let Some(&node) = self.nodes.get(&track_index) else {
            return Vec::new();
        };

        let mut ranked: Vec<(u32, f64)> = self
            .graph
            .edges(node)
            .map(|edge| {
                // an undirected edge can store our node as either end
                let other = if edge.source() == node {
                    edge.target()
                } else {
                    edge.source()
                };
                (self.graph[other], *edge.weight())
            })
            .collect();

        // most similar first; ties are broken by track_index
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(limit);
        ranked
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

/// Removes repeated tracks (keeping the first occurrence) and applies the cap.
fn unique_tracks(tracks: &[u32], limit: usize) -> Vec<u32> {
    let mut seen = HashSet::new();
    tracks
        .iter()
        .copied()
        .filter(|track| seen.insert(*track))
        .take(limit)
        .collect()
}

fn node_for(
    graph: &mut UnGraph<u32, f64>,
    nodes: &mut HashMap<u32, NodeIndex>,
    track: u32,
) -> NodeIndex {
    *nodes.entry(track).or_insert_with(|| graph.add_node(track))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::session;

    fn lenient() -> CoListeningConfig {
        CoListeningConfig {
            min_cooccurrence: 1,
            min_jaccard: 0.0,
            max_session_tracks: 100,
        }
    }

    fn sample_sessions() -> Vec<Session> {
        vec![session(&[1, 2, 3]), session(&[1, 2]), session(&[1, 3])]
    }

    #[test]
    fn similarity_is_jaccard_of_shared_sessions() {
        let graph = CoListeningGraph::build(&sample_sessions(), &lenient());

        // track 1 is in 3 sessions, 2 in 2, 3 in 2; 1&2 share 2, 1&3 share 2
        let similar = graph.similar(1, 5);
        assert_eq!(similar.len(), 2);
        assert_eq!(similar[0].0, 2);
        assert!((similar[0].1 - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(similar[1].0, 3);
        assert!((similar[1].1 - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn repeated_track_in_one_session_counts_once() {
        let graph = CoListeningGraph::build(&[session(&[1, 1, 2])], &lenient());
        assert_eq!(graph.similar(1, 5), vec![(2, 1.0)]);
    }

    #[test]
    fn min_cooccurrence_drops_rare_pairs() {
        let config = CoListeningConfig {
            min_cooccurrence: 2,
            ..lenient()
        };
        let graph = CoListeningGraph::build(&sample_sessions(), &config);

        // pair (2, 3) shares only one session, so it has no edge
        assert_eq!(graph.edge_count(), 2);
        assert_eq!(graph.similar(2, 5).len(), 1);
        assert_eq!(graph.similar(3, 5).len(), 1);
    }

    #[test]
    fn min_jaccard_drops_weak_pairs() {
        let config = CoListeningConfig {
            min_jaccard: 0.5,
            ..lenient()
        };
        let graph = CoListeningGraph::build(&sample_sessions(), &config);

        // pair (2, 3) has similarity 1/3, below the threshold
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn long_sessions_are_capped() {
        let config = CoListeningConfig {
            max_session_tracks: 2,
            ..lenient()
        };
        let graph = CoListeningGraph::build(&[session(&[1, 2, 3, 4])], &config);

        assert_eq!(graph.similar(1, 5), vec![(2, 1.0)]);
        assert!(graph.similar(3, 5).is_empty());
    }

    #[test]
    fn unknown_track_gives_empty_result() {
        let graph = CoListeningGraph::build(&sample_sessions(), &lenient());
        assert!(graph.similar(99, 5).is_empty());
    }
}
