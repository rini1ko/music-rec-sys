use utoipa::OpenApi;

use crate::api::compare::{ComparedItemDto, ComparisonResponse, Movement};
use crate::api::dto::{
    ArtistDto, ArtistProfileDto, CountryPlaysDto, DayPlaysDto, ErrorBody, EventDto, HealthDto,
    HistoryEntryDto, HistoryResponse, NewEvent, NewUser, OverviewDto, RadioResponse, ReasonDto,
    RecommendationDto, RecommendationsResponse, TrackDto, TrackPlaysDto, UserDto, UserSummaryDto,
    UsersResponse, WeightsDto,
};
use crate::api::evaluation::{EvaluationStatus, ModelScoreDto, StabilityDto, WindowDto};
use crate::api::feedback::{
    ArmDto, FeedbackDto, FeedbackItemDto, FeedbackKindDto, FeedbackResponse, NewFeedback,
};
use crate::api::state::ModelStatus;
use crate::api::{compare, evaluation, feedback, handlers, insights, metrics, stream};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Music recommendation API",
        description = "Hybrid recommendations: trends, Markov chain, co-listening graph and collaborative filtering."
    ),
    paths(
        handlers::health,
        handlers::recommendations,
        handlers::add_event,
        handlers::user,
        handlers::model_status,
        handlers::retrain,
        evaluation::start_evaluation,
        evaluation::evaluation,
        feedback::add_feedback,
        feedback::user_feedback,
        metrics::metrics,
        stream::event_stream,
        compare::compare,
        handlers::list_users,
        handlers::search_tracks,
        handlers::create_user,
        handlers::similar_tracks,
        insights::track,
        insights::overview,
        insights::search_artists,
        insights::artist,
        insights::user_history,
        insights::radio,
    ),
    components(schemas(
        HealthDto,
        ErrorBody,
        RecommendationsResponse,
        RecommendationDto,
        ReasonDto,
        NewEvent,
        EventDto,
        UserDto,
        TrackDto,
        ModelStatus,
        WeightsDto,
        EvaluationStatus,
        ModelScoreDto,
        WindowDto,
        StabilityDto,
        NewFeedback,
        FeedbackKindDto,
        FeedbackDto,
        FeedbackItemDto,
        FeedbackResponse,
        ArmDto,
        Movement,
        ComparedItemDto,
        ComparisonResponse,
        UsersResponse,
        UserSummaryDto,
        NewUser,
        OverviewDto,
        TrackPlaysDto,
        ArtistDto,
        DayPlaysDto,
        CountryPlaysDto,
        ArtistProfileDto,
        HistoryResponse,
        HistoryEntryDto,
        RadioResponse,
    ))
)]
pub struct ApiDoc;
