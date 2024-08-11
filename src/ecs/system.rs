use crate::ecs::{Storage, World};

/// Base trait for a subsystem of the engine. Systems are things that operate on entities and are periodically
/// updated. Examples are a rendering system that draws entities to the screen, a physics system that performs
/// physical simulation of entities, an input system that handles mouse/keyboard input, but also game-specific
/// systems that spawn enemies, advance game state etc.
pub trait System {
    fn new() -> Self
    where
        Self: Sized;

    fn update(&mut self, storage: &mut Storage);
}

impl World {
    /// Add a new system statically. The world starts with no default systems for full flexibility.
    ///
    /// # Example
    ///
    /// ```
    /// use game_engine::ecs::{System, Storage, World};
    ///
    /// struct MySystem;
    ///
    /// impl System for MySystem {
    ///     fn new() -> Self {
    ///         Self
    ///     }
    ///
    ///     fn update(&mut self, storage: &mut Storage) {
    ///         // Do something
    ///     }
    /// }
    ///
    /// let mut world = World::init().expect("Failed to initialize world");
    /// world.add_system(MySystem::new());
    /// ```
    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }
}
