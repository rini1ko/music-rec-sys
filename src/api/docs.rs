use utoipa::OpenApi;

use crate::api::dto::{
    ErrorBody, EventDto, HealthDto, NewEvent, ReasonDto, RecommendationDto,
    RecommendationsResponse, TrackDto, UserDto,
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
    ))
)]
pub struct ApiDoc;