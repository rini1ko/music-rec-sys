use utoipa::OpenApi;

use crate::api::dto::{
    ArtistDto, ArtistProfileDto, CountryPlaysDto, DayPlaysDto, ErrorBody, EventDto, HealthDto,
    HistoryEntryDto, HistoryResponse, NewEvent, NewUser, OverviewDto, RadioResponse, ReasonDto,
    RecommendationDto, RecommendationsResponse, TrackDto, TrackPlaysDto, UserDto, UserSummaryDto,
    UsersResponse, WeightsDto,
};
use crate::api::state::ModelStatus;
use crate::api::{handlers, insights};

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
