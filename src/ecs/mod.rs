//! # ECS Core
//! This module contains the core functionality of the engine. It is based on the ECS pattern and is
//! therefore responsible for managing the game world, entities, components, systems and queries.
//!
//! We use the following terminology:
//! - `Entity`: An entity is a unique identifier that groups components together. It is a simple
//!  [number](EntityId).
//! - `Component`: A component is a piece of data that is attached to an entity. It is possible to
//! attach an arbitrary type as a component, as long as the lifetimes of all members of the
//! component are `'static`. This is possible since the engine uses a dynamic type system
//! for components.
//! - [`System`]: A system is something that operates on entities that share a certain set of
//! components. There are some predefined systems in the engine, but it is also possible to create
//! custom systems. The methods in the [`Query`] trait are used to filter entities based on their
//! components.
//! - [`World`]: The world is the main struct that holds all the entities, components and
//! systems. It is responsible for updating the systems and handling the general game loop. The
//! actual housekeeping of entities, components and systems is done by the [`Storage`] struct, that
//! will be accessible from each system.
mod archetype;
mod entity_builder;
mod query;
mod storage;
mod system;
mod world;

pub use entity_builder::EntityBuilder;
pub use query::Query;
pub use storage::Storage;
pub use system::System;
pub use world::*;
