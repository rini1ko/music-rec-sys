/// Which model produced a candidate. Used for weights and explanations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Trends,
    Markov,
    CoListening,
    Cf,
    /// Matrix factorization (R8).
    Mf,
    Artist,
    Country,
    Popular,
}

impl Source {
    /// Human-readable reason shown next to a recommendation.
    pub fn reason(self) -> &'static str {
        match self {
            Source::Trends => "popular right now",
            Source::Markov => "often played right after your last track",
            Source::CoListening => "often listened to together with your recent tracks",
            Source::Cf => "people with similar taste listened to this",
            Source::Mf => "fits the taste hidden in your plays",
            Source::Artist => "you often listen to this artist",
            Source::Country => "popular in your country",
            Source::Popular => "popular among all listeners",
        }
    }

    /// Stable machine name of the model, used in the API.
    pub fn name(self) -> &'static str {
        match self {
            Source::Trends => "trends",
            Source::Markov => "markov",
            Source::CoListening => "co_listening",
            Source::Cf => "cf",
            Source::Mf => "mf",
            Source::Artist => "artist",
            Source::Country => "country",
            Source::Popular => "popular",
        }
    }

    /// The source with this [`Source::name`], if there is one.
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Source::Trends,
            Source::Markov,
            Source::CoListening,
            Source::Cf,
            Source::Mf,
            Source::Artist,
            Source::Country,
            Source::Popular,
        ]
        .into_iter()
        .find(|source| source.name() == name)
    }
}

/// A track proposed by one model. Scores of different models are not comparable
/// (play counts vs probabilities vs similarities), the ranker normalizes them.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub track_index: u32,
    pub score: f64,
}

pub struct RecommendationContext {
    /// Tracks the user listened to, oldest first.
    pub history: Vec<u32>,
}

pub trait CandidateGenerator: Send + Sync {
    fn source(&self) -> Source;
    fn candidates(&self, context: &RecommendationContext, limit: usize) -> Vec<Candidate>;

    /// How much the model keeps (transitions, graph edges, users...), for
    /// `/metrics`. Generators that keep nothing worth counting return 0.
    fn size(&self) -> usize {
        0
    }
}
