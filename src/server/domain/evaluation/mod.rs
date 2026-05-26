pub mod dataset;
pub mod ids;
pub mod optimizer;
pub mod question;
pub mod ranking;
pub mod run;
pub mod scoring;
pub mod split;
pub mod value_objects;

use crate::server::domain::comprehension::map_item::MapItemRef;
use crate::shared::contracts::MapItemRefDto;

pub fn map_item_ref_to_dto(r: MapItemRef) -> MapItemRefDto {
    match r {
        MapItemRef::Observation {
            map_id,
            observation_id,
        } => MapItemRefDto::Observation {
            map_id,
            observation_id,
        },
        MapItemRef::Thread { map_id, thread_id } => MapItemRefDto::Thread { map_id, thread_id },
        MapItemRef::Insight { map_id, insight_id } => MapItemRefDto::Insight { map_id, insight_id },
        MapItemRef::Connection {
            corpus_map_id,
            connection_id,
        } => MapItemRefDto::Connection {
            corpus_map_id,
            connection_id,
        },
    }
}
