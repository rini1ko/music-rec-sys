//! `export` (E6): listeners with their play counts, or the recommendations of
//! every listener, as CSV or JSON, for spreadsheets and other tools.

use std::collections::HashMap;
use std::io::Write;

use serde::Serialize;
use thiserror::Error;

use crate::db::error::DbError;
use crate::db::repository::Repository;
use crate::engine::{EngineConfig, build_ranker};
use crate::models::catalog::Catalog;
use crate::recommender::{Recommender, WeightOverrides};

#[derive(Debug, Error)]
pub enum ExportError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("cannot write CSV")]
    Csv(#[from] csv::Error),
    #[error("cannot write JSON")]
    Json(#[from] serde_json::Error),
    #[error("cannot write the output")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserRow {
    pub user_index: u32,
    pub gender: Option<String>,
    pub age: Option<f32>,
    pub country: Option<String>,
    pub plays: usize,
}

/// One recommended track of one listener; `reasons` are the source names,
/// strongest first, joined with `+` so a CSV cell holds them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecommendationRow {
    pub user_index: u32,
    pub rank: usize,
    pub track_index: u32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub score: f64,
    pub reasons: String,
}

/// Every listener with the number of their plays, most active first.
pub fn users(repo: &(impl Repository + ?Sized)) -> Result<Vec<UserRow>, DbError> {
    let mut plays: HashMap<u32, usize> = HashMap::new();
    for event in repo.all_events()? {
        *plays.entry(event.user_index).or_default() += 1;
    }
    let mut rows: Vec<UserRow> = repo
        .all_users()?
        .into_iter()
        .map(|user| UserRow {
            plays: plays.get(&user.user_index).copied().unwrap_or(0),
            user_index: user.user_index,
            gender: user.gender,
            age: user.age,
            country: user.country,
        })
        .collect();
    rows.sort_by(|a, b| b.plays.cmp(&a.plays).then(a.user_index.cmp(&b.user_index)));
    Ok(rows)
}

/// The `top` recommendations of every listener, with the rules the API uses.
pub fn recommendations(
    repo: &(impl Repository + ?Sized),
    config: &EngineConfig,
    top: usize,
) -> Result<Vec<RecommendationRow>, DbError> {
    let tracks = repo.all_tracks()?;
    let users = repo.all_users()?;
    let events = repo.all_events()?;
    let ranker = build_ranker(repo, config)?;
    let catalog = Catalog::build(&tracks, &users, &events, config.recommender.cold_start_pool);
    let recommender = Recommender::new(ranker, catalog, config.recommender.clone());
    let names: HashMap<u32, _> = tracks
        .into_iter()
        .map(|track| (track.track_index, track))
        .collect();

    let mut rows = Vec::new();
    for user in users {
        let history = repo.user_history(user.user_index)?;
        let list = recommender.recommend(
            history,
            user.country.as_deref(),
            top,
            WeightOverrides::default(),
        );
        for (position, item) in list.into_iter().enumerate() {
            let track = names.get(&item.track_index);
            rows.push(RecommendationRow {
                user_index: user.user_index,
                rank: position + 1,
                track_index: item.track_index,
                title: track.and_then(|track| track.track_name.clone()),
                artist: track.and_then(|track| track.artist_name.clone()),
                score: item.score,
                reasons: item
                    .reasons
                    .iter()
                    .map(|reason| reason.source.name())
                    .collect::<Vec<_>>()
                    .join("+"),
            });
        }
    }
    Ok(rows)
}

/// Writes the rows as CSV with a header row, or as a pretty JSON array.
pub fn write<T: Serialize>(rows: &[T], format: Format, out: impl Write) -> Result<(), ExportError> {
    match format {
        Format::Csv => {
            let mut writer = csv::Writer::from_writer(out);
            for row in rows {
                writer.serialize(row)?;
            }
            writer.flush()?;
        }
        Format::Json => {
            let mut out = out;
            serde_json::to_writer_pretty(&mut out, rows)?;
            writeln!(out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory::MemoryRepository;
    use crate::domain::test_utils::{event_of, named_track, session_of, user};

    fn repo() -> MemoryRepository {
        let mut repo = MemoryRepository::default();
        repo.add_users(vec![user(1), user(2), user(3)]).unwrap();
        repo.add_tracks(vec![
            named_track(10, 1, "Crocodile", "Underworld"),
            named_track(11, 1, "Rez", "Underworld"),
            named_track(12, 2, "Kids", "MGMT"),
        ])
        .unwrap();
        repo.add_events(vec![
            event_of(1, 10, 0),
            event_of(1, 11, 1),
            event_of(2, 10, 0),
            event_of(2, 11, 1),
            event_of(2, 12, 2),
        ])
        .unwrap();
        repo.add_sessions(vec![session_of(1, &[10, 11]), session_of(2, &[10, 11, 12])])
            .unwrap();
        repo
    }

    #[test]
    fn users_come_most_active_first_with_play_counts() {
        let rows = users(&repo()).unwrap();
        let summary: Vec<(u32, usize)> =
            rows.iter().map(|row| (row.user_index, row.plays)).collect();
        assert_eq!(summary, vec![(2, 3), (1, 2), (3, 0)]);
    }

    #[test]
    fn every_listener_gets_ranked_tracks_with_names() {
        let rows = recommendations(&repo(), &EngineConfig::lenient(), 5).unwrap();
        // user 1 has not heard 12 yet, user 3 gets the cold start
        let first = rows.iter().find(|row| row.user_index == 1).unwrap();
        assert_eq!((first.rank, first.track_index), (1, 12));
        assert_eq!(first.artist.as_deref(), Some("MGMT"));
        assert!(!first.reasons.is_empty());
        assert!(
            rows.iter()
                .any(|row| row.user_index == 3 && row.reasons == "popular")
        );
    }

    #[test]
    fn rows_are_written_as_csv_or_json() {
        let rows = users(&repo()).unwrap();
        let mut csv = Vec::new();
        write(&rows, Format::Csv, &mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        assert!(text.starts_with("user_index,gender,age,country,plays\n2,,,,3\n"));

        let mut json = Vec::new();
        write(&rows, Format::Json, &mut json).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(parsed[0]["plays"], 3);
    }
}
