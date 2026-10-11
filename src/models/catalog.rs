use std::collections::HashMap;

use crate::domain::{ListenEvent, Track, User};

/// Facts the models do not use, but the rules around them do:
/// who made a track and what is popular where.
pub struct Catalog {
    artist_of: HashMap<u32, u32>,
    popular_in_country: HashMap<String, Vec<(u32, usize)>>,
    popular_overall: Vec<(u32, usize)>,
}

impl Catalog {
    /// `pool` is how many popular tracks to keep for each country and overall.
    pub fn build(tracks: &[Track], users: &[User], events: &[ListenEvent], pool: usize) -> Self {
        let artist_of = tracks
            .iter()
            .map(|track| (track.track_index, track.artist_index))
            .collect();

        let country_of: HashMap<u32, &str> = users
            .iter()
            .filter_map(|user| {
                user.country
                    .as_deref()
                    .map(|country| (user.user_index, country))
            })
            .collect();

        let mut overall: HashMap<u32, usize> = HashMap::new();
        let mut by_country: HashMap<&str, HashMap<u32, usize>> = HashMap::new();
        for event in events {
            *overall.entry(event.track_index).or_default() += 1;
            if let Some(&country) = country_of.get(&event.user_index) {
                *by_country
                    .entry(country)
                    .or_default()
                    .entry(event.track_index)
                    .or_default() += 1;
            }
        }

        Catalog {
            artist_of,
            popular_in_country: by_country
                .into_iter()
                .map(|(country, counts)| (country.to_string(), top(counts, pool)))
                .collect(),
            popular_overall: top(overall, pool),
        }
    }

    pub fn artist_of(&self, track_index: u32) -> Option<u32> {
        self.artist_of.get(&track_index).copied()
    }

    /// Most played first. Empty if nobody from that country listened to anything.
    pub fn popular_in(&self, country: &str) -> &[(u32, usize)] {
        self.popular_in_country
            .get(country)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn popular_overall(&self) -> &[(u32, usize)] {
        &self.popular_overall
    }
}

/// Most played first, ties by track index.
fn top(counts: HashMap<u32, usize>, pool: usize) -> Vec<(u32, usize)> {
    let mut ranked: Vec<(u32, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(pool);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_utils::{event_of, user};

    fn from(index: u32, country: &str) -> User {
        User {
            country: Some(country.to_string()),
            ..user(index)
        }
    }

    #[test]
    fn popularity_is_counted_per_country_and_overall() {
        let users = [from(1, "UK"), from(2, "UK"), from(3, "DE")];
        let events = [
            event_of(1, 10, 0),
            event_of(2, 10, 1),
            event_of(1, 11, 2),
            event_of(3, 11, 0),
            event_of(3, 12, 1),
        ];
        let catalog = Catalog::build(&[], &users, &events, 5);

        assert_eq!(catalog.popular_in("UK"), &[(10, 2), (11, 1)]);
        assert_eq!(catalog.popular_in("DE"), &[(11, 1), (12, 1)]);
        assert_eq!(catalog.popular_overall(), &[(10, 2), (11, 2), (12, 1)]);
        assert!(catalog.popular_in("FR").is_empty());
    }

    #[test]
    fn pool_limits_the_lists() {
        let users = [from(1, "UK")];
        let events = [event_of(1, 10, 0), event_of(1, 11, 1), event_of(1, 12, 2)];
        let catalog = Catalog::build(&[], &users, &events, 2);

        assert_eq!(catalog.popular_overall().len(), 2);
        assert_eq!(catalog.popular_in("UK").len(), 2);
    }

    #[test]
    fn artists_are_looked_up_by_track() {
        let tracks = [Track {
            track_index: 7,
            artist_index: 3,
            track_name: None,
            artist_name: None,
        }];
        let catalog = Catalog::build(&tracks, &[], &[], 5);

        assert_eq!(catalog.artist_of(7), Some(3));
        assert_eq!(catalog.artist_of(8), None);
    }
}
