use crate::ecs::{Storage, System};

/// A unique id for an entity
pub type EntityId = usize;

/// The main struct that holds all the game state. The storage is responsible for managing the
/// entities and components. The storage is then passed into every system.
pub struct World {
    pub(crate) systems: Vec<Box<dyn System>>,
    pub storage: Storage,
    pub(crate) entities_count: EntityId,
    // TODO: replace ggez dependencies with winit window loop and custom game loop logic
    // pub(crate) ggez_context: ggez::Context,
    // pub(crate) event_loop: EventLoop<()>,
}

impl World {
    /// Create a new entity and return its ID
    pub(crate) fn new_entity(&mut self) -> EntityId {
        let entity_id = self.entities_count;

        self.entities_count += 1;
        entity_id
    }
}
