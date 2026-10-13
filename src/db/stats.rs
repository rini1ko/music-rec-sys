//! Statistics for the dashboard (E1). Everything is counted by SQLite with
//! GROUP BY over the events of a time window that ends at the newest event.

use chrono::{Duration, NaiveDate};
use sea_orm::{DatabaseConnection, FromQueryResult, Value};

use crate::db::error::DbError;
use crate::db::sea_repository::latest_event_time;
use crate::db::sql::{ArtistRow, TrackPlaysRow, bound, count, statement};
use crate::domain::{CountryPlays, DayPlays, StatsOverview};

#[derive(Debug, FromQueryResult)]
struct TotalsRow {
    plays: i64,
    listeners: i64,
    tracks: i64,
    artists: i64,
}

#[derive(Debug, FromQueryResult)]
struct DayRow {
    day: String,
    plays: i64,
}

#[derive(Debug, FromQueryResult)]
struct SlotRow {
    slot: i64,
    plays: i64,
}

#[derive(Debug, FromQueryResult)]
struct CountryRow {
    country: Option<String>,
    listeners: i64,
    plays: i64,
}

const TOTALS: &str = "
    SELECT COUNT(*) AS plays,
           COUNT(DISTINCT e.user_index) AS listeners,
           COUNT(DISTINCT e.track_index) AS tracks,
           COUNT(DISTINCT t.artist_index) AS artists
    FROM listen_events e
    JOIN tracks t ON t.track_index = e.track_index
    WHERE e.timestamp >= ?";

const TOP_TRACKS: &str = "
    SELECT e.track_index, t.artist_index, t.track_name, t.artist_name, COUNT(*) AS plays
    FROM listen_events e
    JOIN tracks t ON t.track_index = e.track_index
    WHERE e.timestamp >= ?
    GROUP BY e.track_index
    ORDER BY plays DESC, e.track_index
    LIMIT ?";

const TOP_ARTISTS: &str = "
    SELECT t.artist_index, MAX(t.artist_name) AS artist_name,
           COUNT(DISTINCT e.track_index) AS tracks,
           COUNT(*) AS plays,
           COUNT(DISTINCT e.user_index) AS listeners
    FROM listen_events e
    JOIN tracks t ON t.track_index = e.track_index
    WHERE e.timestamp >= ?
    GROUP BY t.artist_index
    ORDER BY plays DESC, t.artist_index
    LIMIT ?";

const BY_DAY: &str = "
    SELECT date(timestamp) AS day, COUNT(*) AS plays
    FROM listen_events
    WHERE timestamp >= ?
    GROUP BY day
    ORDER BY day";

// strftime('%H') is the hour 00-23, strftime('%w') the weekday 0-6 with Sunday as 0
const BY_HOUR: &str = "
    SELECT CAST(strftime('%H', timestamp) AS INTEGER) AS slot, COUNT(*) AS plays
    FROM listen_events
    WHERE timestamp >= ?
    GROUP BY slot";

const BY_WEEKDAY: &str = "
    SELECT CAST(strftime('%w', timestamp) AS INTEGER) AS slot, COUNT(*) AS plays
    FROM listen_events
    WHERE timestamp >= ?
    GROUP BY slot";

const COUNTRIES: &str = "
    SELECT u.country, COUNT(DISTINCT e.user_index) AS listeners, COUNT(*) AS plays
    FROM listen_events e
    JOIN users u ON u.user_index = e.user_index
    WHERE e.timestamp >= ?
    GROUP BY u.country
    ORDER BY plays DESC, u.country
    LIMIT ?";

/// Statistics of the last `days` days of the data. `top` limits the lists
/// of tracks, artists and countries.
pub(crate) async fn overview(
    db: &DatabaseConnection,
    days: u32,
    top: usize,
) -> Result<StatsOverview, DbError> {
    let Some(latest) = latest_event_time(db).await? else {
        return Ok(empty_overview());
    };
    let start = latest - Duration::days(i64::from(days));
    let since = || Value::from(start);

    let totals = TotalsRow::find_by_statement(statement(db, TOTALS, [since()]))
        .one(db)
        .await?;
    let top_tracks =
        TrackPlaysRow::find_by_statement(statement(db, TOP_TRACKS, [since(), bound(top)]))
            .all(db)
            .await?;
    let top_artists =
        ArtistRow::find_by_statement(statement(db, TOP_ARTISTS, [since(), bound(top)]))
            .all(db)
            .await?;
    let days_rows = DayRow::find_by_statement(statement(db, BY_DAY, [since()]))
        .all(db)
        .await?;
    let hours = SlotRow::find_by_statement(statement(db, BY_HOUR, [since()]))
        .all(db)
        .await?;
    let weekdays = SlotRow::find_by_statement(statement(db, BY_WEEKDAY, [since()]))
        .all(db)
        .await?;
    let countries = CountryRow::find_by_statement(statement(db, COUNTRIES, [since(), bound(top)]))
        .all(db)
        .await?;

    let totals = totals.unwrap_or(TotalsRow {
        plays: 0,
        listeners: 0,
        tracks: 0,
        artists: 0,
    });
    Ok(StatsOverview {
        window: Some((start, latest)),
        plays: count(totals.plays),
        listeners: count(totals.listeners),
        tracks: count(totals.tracks),
        artists: count(totals.artists),
        top_tracks: top_tracks.into_iter().map(Into::into).collect(),
        top_artists: top_artists.into_iter().map(Into::into).collect(),
        by_day: every_day(
            days_rows
                .into_iter()
                .map(day_plays)
                .collect::<Result<_, _>>()?,
            start.date(),
            latest.date(),
        ),
        by_hour: spread(hours),
        by_weekday: spread(weekdays),
        countries: countries
            .into_iter()
            .map(|row| CountryPlays {
                country: row.country,
                listeners: count(row.listeners),
                plays: count(row.plays),
            })
            .collect(),
    })
}

fn empty_overview() -> StatsOverview {
    StatsOverview {
        window: None,
        plays: 0,
        listeners: 0,
        tracks: 0,
        artists: 0,
        top_tracks: Vec::new(),
        top_artists: Vec::new(),
        by_day: Vec::new(),
        by_hour: [0; 24],
        by_weekday: [0; 7],
        countries: Vec::new(),
    }
}

/// One entry per day from `first` to `last`, with 0 for the days SQLite had
/// no row for: a quiet day must show as a gap, not vanish from the chart.
fn every_day(counted: Vec<DayPlays>, first: NaiveDate, last: NaiveDate) -> Vec<DayPlays> {
    let plays: std::collections::HashMap<NaiveDate, usize> = counted
        .into_iter()
        .map(|item| (item.day, item.plays))
        .collect();
    first
        .iter_days()
        .take_while(|day| *day <= last)
        .map(|day| DayPlays {
            day,
            plays: plays.get(&day).copied().unwrap_or(0),
        })
        .collect()
}

fn day_plays(row: DayRow) -> Result<DayPlays, DbError> {
    let day = NaiveDate::parse_from_str(&row.day, "%Y-%m-%d")
        .map_err(|error| DbError::Storage(format!("bad day {:?} from SQLite: {error}", row.day)))?;
    Ok(DayPlays {
        day,
        plays: count(row.plays),
    })
}

/// Puts the counts of hours or weekdays into their slots. Slots outside the array are ignored.
fn spread<const N: usize>(rows: Vec<SlotRow>) -> [usize; N] {
    let mut slots = [0; N];
    for row in rows {
        if let Some(slot) = usize::try_from(row.slot)
            .ok()
            .and_then(|index| slots.get_mut(index))
        {
            *slot = count(row.plays);
        }
    }
    slots
}

#[cfg(test)]
mod tests {
    use crate::db::store::Store;
    use crate::domain::test_utils::{event_of, named_track, user, user_from};

    const DAY: i64 = 24 * 60;

    /// User 1 (UK) plays track 10 twice and 11 once, user 2 (no country) plays
    /// 12 once. Track 12 is three days before the others.
    async fn store() -> Store {
        let store = Store::open("sqlite::memory:").await.unwrap();
        store
            .add_users(vec![user_from(1, "UK"), user(2)])
            .await
            .unwrap();
        store
            .add_tracks(vec![
                named_track(10, 100, "Crocodile", "Underworld"),
                named_track(11, 100, "Born Slippy", "Underworld"),
                named_track(12, 200, "Kids", "MGMT"),
            ])
            .await
            .unwrap();
        store
            .add_events(vec![
                event_of(2, 12, 0),
                event_of(1, 10, 3 * DAY),
                event_of(1, 10, 3 * DAY + 60),
                event_of(1, 11, 3 * DAY + 61),
            ])
            .await
            .unwrap();
        store
    }

    #[tokio::test]
    async fn window_ends_at_the_newest_event() {
        let overview = store().await.overview(1, 10).await.unwrap();

        let (from, to) = overview.window.unwrap();
        assert_eq!(to - from, chrono::Duration::days(1));
        // track 12 is older than one day, so it is left out
        assert_eq!(
            (
                overview.plays,
                overview.listeners,
                overview.tracks,
                overview.artists
            ),
            (3, 1, 2, 1)
        );
        assert_eq!(overview.top_tracks[0].track.track_index, 10);
        assert_eq!(overview.top_tracks[0].plays, 2);
        assert_eq!(
            overview.top_artists[0].artist_name.as_deref(),
            Some("Underworld")
        );
        assert_eq!(overview.top_artists[0].tracks, 2);
    }

    #[tokio::test]
    async fn plays_are_split_by_day_hour_and_weekday() {
        let overview = store().await.overview(7, 10).await.unwrap();

        assert_eq!(overview.plays, 4);
        let days: Vec<(String, usize)> = overview
            .by_day
            .iter()
            .map(|item| (item.day.to_string(), item.plays))
            .collect();
        // every day of the window is there, quiet ones with 0
        assert_eq!(days.len(), 8);
        assert_eq!(days[0], ("2009-04-30".to_string(), 0));
        assert_eq!(days[4], ("2009-05-04".to_string(), 1));
        assert_eq!(days[5], ("2009-05-05".to_string(), 0));
        assert_eq!(days[7], ("2009-05-07".to_string(), 3));
        // the events are at 12:00, 12:00, 13:00 and 13:01
        assert_eq!(overview.by_hour[12], 2);
        assert_eq!(overview.by_hour[13], 2);
        assert_eq!(overview.by_hour.iter().sum::<usize>(), 4);
        // 2009-05-04 was a Monday, 2009-05-07 a Thursday
        assert_eq!(overview.by_weekday[1], 1);
        assert_eq!(overview.by_weekday[4], 3);
    }

    #[tokio::test]
    async fn countries_keep_listeners_without_a_country() {
        let overview = store().await.overview(7, 10).await.unwrap();
        let countries: Vec<(Option<&str>, usize, usize)> = overview
            .countries
            .iter()
            .map(|item| (item.country.as_deref(), item.listeners, item.plays))
            .collect();
        assert_eq!(countries, vec![(Some("UK"), 1, 3), (None, 1, 1)]);
    }

    #[tokio::test]
    async fn top_lists_are_cut() {
        let overview = store().await.overview(7, 1).await.unwrap();
        assert_eq!(overview.top_tracks.len(), 1);
        assert_eq!(overview.top_artists.len(), 1);
        assert_eq!(overview.countries.len(), 1);
    }

    #[tokio::test]
    async fn empty_database_gives_empty_statistics() {
        let store = Store::open("sqlite::memory:").await.unwrap();
        let overview = store.overview(30, 10).await.unwrap();
        assert_eq!(overview.window, None);
        assert_eq!(overview.plays, 0);
        assert!(overview.by_day.is_empty());
    }
}
