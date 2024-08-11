use crate::ecs::{EntityId, World};
use std::marker::PhantomData;

#[derive(Default, Clone)]
pub struct NoComponents;
#[derive(Default, Clone)]
pub struct HasComponents;

/// Builder pattern for entities. Provides a fail-safe API for entity-creation using the Builder and
/// [TypeState](https://cliffle.com/blog/rust-typestate) patterns, which ensure that entities are
/// built with at least one component. This is useful since entities without components are useless
/// in the ECS pattern.
///
/// Usage:
/// ```
/// use game_engine::ecs::World;
///
/// struct Name(&'static str);
/// struct Health(i32);
///
/// let mut world = World::init().expect("Failed to initialize world");
///
/// let _ = world.build_entity()
///     .with_component::<Name>(Name("Player"))
///     .with_component::<Health>(Health(100))
///     .build();
/// ```
pub struct EntityBuilder<'a, C> {
    world: &'a mut World,
    entity_id: EntityId,
    marker_has_components: PhantomData<C>,
}

impl<'a> EntityBuilder<'a, NoComponents> {
    fn new(world: &'a mut World) -> Self {
        let entity_id = world.new_entity();

        Self {
            world,
            entity_id,
            marker_has_components: PhantomData,
        }
    }

    pub fn with_component<C: 'static>(self, component: C) -> EntityBuilder<'a, HasComponents> {
        self.world
            .storage
            .add_component_to_entity(self.entity_id, component);

        EntityBuilder {
            world: self.world,
            entity_id: self.entity_id,
            marker_has_components: PhantomData,
        }
    }
}

impl EntityBuilder<'_, HasComponents> {
    #[must_use]
    pub fn with_component<C: 'static>(self, component: C) -> Self {
        self.world
            .storage
            .add_component_to_entity(self.entity_id, component);

        self
    }

    #[must_use]
    pub const fn build(self) -> EntityId {
        self.entity_id
    }
}

impl World {
    pub fn build_entity(&mut self) -> EntityBuilder<NoComponents> {
        EntityBuilder::new(self)
    }
}
