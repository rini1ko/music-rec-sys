use utoipa::OpenApi;

use crate::api::dto::{
    ErrorBody, EventDto, HealthDto, NewEvent, ReasonDto, RecommendationDto,
    RecommendationsResponse, TrackDto, UserDto, UserSummaryDto, UsersResponse, NewUser
};
use crate::api::handlers;
use crate::api::state::ModelStatus;

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
        UsersResponse,
        UserSummaryDto,
        NewUser,
    ))
)]
pub struct ApiDoc;