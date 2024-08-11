use crate::ecs::archetype::{align_and_migrate_archetypes, Archetype, ArchetypeId};
use crate::ecs::EntityId;
use std::any::{Any, TypeId};
use std::collections::HashMap;

pub trait ComponentVec: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn new_empty(&self) -> Box<dyn ComponentVec>;
    fn len(&self) -> usize;
    #[allow(dead_code)]
    fn is_empty(&self) -> bool;
    fn element_type_id(&self) -> TypeId;
    fn migrate_element(&mut self, index: usize, other: &mut dyn ComponentVec);
    fn swap_remove(&mut self, index: usize);
}

impl<T: 'static> ComponentVec for Vec<T> {
    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }

    fn new_empty(&self) -> Box<dyn ComponentVec> {
        Box::<Self>::default()
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn element_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn migrate_element(&mut self, index: usize, other: &mut dyn ComponentVec) {
        let element = self.swap_remove(index);
        if let Some(other) = other.as_any_mut().downcast_mut::<Self>() {
            other.push(element);
        } else {
            panic!(
                "Type mismatch during migration: expected {:?}",
                std::any::type_name::<T>()
            );
        }
    }

    fn swap_remove(&mut self, index: usize) {
        self.swap_remove(index);
    }
}

/// An index to the row in an archetype that stores the components of an entity.
pub type EntityRow = usize;

/// A record of an entity in an archetype. This is used inside the `entity_index` to keep track of
///  a) which archetype an entity belongs to and
///  b) which row in the archetype the components of the entity are stored
struct EntityRecord {
    pub(crate) archetype_id: ArchetypeId,
    pub(crate) entity_row: EntityRow,
}

/// The storage struct is responsible for managing the entities and components of the game world.
/// `Archetypes` are used to group entities with the same components together, but are generally only
/// used internally.
pub struct Storage {
    /// Vector of all archetypes in the storage. The index in the vector is the archetype id.
    pub(crate) archetypes: HashMap<ArchetypeId, Archetype>,
    pub(crate) component_index: HashMap<TypeId, Vec<ArchetypeId>>,
    entity_index: HashMap<EntityId, EntityRecord>,
    archetype_id_counter: ArchetypeId,
}

impl Storage {
    /// Remove an entity from the Storage. This updates the entities archetype by removing the
    /// swap removing the entity row. Removes the archetype if this is the only entity for this
    /// archetype.
    ///
    /// # Panics
    ///
    /// Panics if the `EntityId` points to an invalid archetype id.
    pub fn remove_entity(&mut self, entity: EntityId) {
        // TODO: check if error handling or feedback is necessary
        let Some(record) = self.entity_index.remove(&entity) else {
            return;
        };

        let archetype = self
            .archetypes
            .get_mut(&record.archetype_id)
            .expect("Internal storage error. Entity index points to invalid archetype id.");

        let archetype_size = archetype.component_types[0].len();

        // remove archetype if it only contains the current entity
        if archetype_size == 1 {
            self.remove_archetype(record.archetype_id);
            return;
        }

        // remove current entity
        archetype.component_types.iter_mut().for_each(|column| {
            column.swap_remove(record.entity_row);
        });

        // we swap_remove the entity row, so all components in the last row are moved to the removed
        // row, meaning we have to update the entity index for the moved entity
        if record.entity_row < archetype_size - 1 {
            let moved_entity = self
                .entity_index
                .iter_mut()
                .find(|(_, r)| r.archetype_id == record.archetype_id)
                .expect("Entity not found.");

            moved_entity.1.entity_row = record.entity_row;
        }
    }

    /// Adds a component to an entity. This will create a new archetype if none exists for the
    /// desired collection of components.
    ///
    /// Components are moved to the new archetype and the entity index is updated.
    ///
    /// # Panics
    ///
    /// Panics if the `EntityId` points to an invalid archetype id.
    pub fn add_component_to_entity<ComponentType: 'static>(
        &mut self,
        entity: EntityId,
        component: ComponentType,
    ) {
        // If a new entity with a missing component is added, we create a new archetype for it
        if !self.has_component::<ComponentType>() && !self.entity_index.contains_key(&entity) {
            let archetype = self.add_archetype_for_new_component_type(component);
            let record = EntityRecord {
                archetype_id: archetype.id,
                entity_row: 0,
            };
            self.entity_index.insert(entity, record);
            return;
        }

        // If the entity already has a component of the same type, we don't need to do anything
        // TODO: maybe we should return an error here? Or simply swap the component?
        if self.entity_index.contains_key(&entity)
            && self.has_entity_component::<ComponentType>(entity)
        {
            return;
        }

        let new_archetype_id = {
            let current_archetype = self.get_archetype_for_entity(entity);

            let mut wanted_component_types = current_archetype
                .map(|archetype| archetype.types.clone())
                .unwrap_or_default();

            wanted_component_types.push(TypeId::of::<ComponentType>());

            if let Some(id) = self
                .find_archetype_id_by_type_ids::<ComponentType>(wanted_component_types.as_slice())
            {
                id
            } else {
                let id = self.archetype_id_counter;
                let new_archetype = Archetype::new_from_add::<ComponentType>(
                    current_archetype.expect("Expected entity with existing archetype."),
                    id,
                );

                self.register_archetype(new_archetype);

                id
            }
        };

        self.move_entity_to_new_archetype(entity, new_archetype_id);

        let new_archetype = self
            .archetypes
            .get_mut(&new_archetype_id)
            .expect("Internal storage error. Invalid Archetype ID.");
        new_archetype.push_component(component);

        // update the entity index
        let new_record = EntityRecord {
            archetype_id: new_archetype.id,
            entity_row: new_archetype.component_types[0].len() - 1,
        };
        self.entity_index.insert(entity, new_record);
    }

    /// Removes a component from an entity. This will create a new archetype if none exists for the
    /// desired collection of components. Since archetypes are never cleaned up this however is
    /// generally going to happen less often than adding components.
    ///
    /// Components are moved to the new archetype and the entity index is updated.
    ///
    /// # Panics
    ///
    /// Panics if the `EntityId` points to an invalid archetype id.
    pub fn remove_component_from_entity<ComponentType: 'static>(
        &mut self,
        entity: EntityId,
        _component: &ComponentType,
    ) {
        if !self.entity_index.contains_key(&entity)
            || !self.has_entity_component::<ComponentType>(entity)
        {
            return;
        }

        let new_archetype_id = {
            let current_archetype = self.get_archetype_for_entity(entity);

            // filter out the type id of the component we want to remove
            let wanted_component_types = current_archetype
                .map(|archetype| archetype.types.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|type_id| *type_id != TypeId::of::<ComponentType>())
                .collect::<Vec<_>>();

            if let Some(id) =
                self.find_archetype_id_by_type_ids::<ComponentType>(&wanted_component_types)
            {
                id
            } else {
                let id = self.archetype_id_counter;
                let new_archetype = Archetype::new_from_remove::<ComponentType>(
                    current_archetype.expect("Expected entity with existing archetype."),
                    id,
                );

                self.register_archetype(new_archetype);

                id
            }
        };

        self.move_entity_to_new_archetype(entity, new_archetype_id);

        let new_archetype = self
            .archetypes
            .get_mut(&new_archetype_id)
            .expect("Internal storage error. Invalid Archetype ID.");

        // update the entity index
        let new_record = EntityRecord {
            archetype_id: new_archetype.id,
            entity_row: new_archetype.component_types[0].len() - 1,
        };
        self.entity_index.insert(entity, new_record);
    }

    pub(crate) fn get_archetype_ids_for_component<ComponentType: 'static>(
        &self,
    ) -> Option<&Vec<ArchetypeId>> {
        self.component_index.get(&TypeId::of::<ComponentType>())
    }

    pub(crate) fn get_archetypes_for_component<ComponentType: 'static>(&self) -> Vec<&Archetype> {
        let archetype_ids = self.get_archetype_ids_for_component::<ComponentType>();

        archetype_ids.map_or_else(Vec::new, |archetype_ids| {
            archetype_ids
                .iter()
                .map(|&id| &self.archetypes[&id])
                .collect()
        })
    }

    pub(crate) fn get_archetypes_for_component_mut<ComponentType: 'static>(
        &mut self,
    ) -> Vec<&mut Archetype> {
        let archetype_ids = self
            .get_archetype_ids_for_component::<ComponentType>()
            .cloned();

        if let Some(archetype_ids) = archetype_ids {
            self.archetypes
                .values_mut()
                .filter(|archetype| archetype_ids.contains(&archetype.id))
                .collect()
        } else {
            vec![]
        }
    }

    fn find_archetype_id_by_type_ids<ComponentType: 'static>(
        &self,
        type_ids: &[TypeId],
    ) -> Option<ArchetypeId> {
        let matching_archetypes = self.get_archetypes_for_component::<ComponentType>();

        matching_archetypes
            .iter()
            .find(|archetype| {
                archetype.types.len() == type_ids.len()
                    && archetype
                        .types
                        .iter()
                        .all(|type_id| type_ids.contains(type_id))
            })
            .map(|archetype| archetype.id)
    }

    fn add_archetype_for_new_component_type<ComponentType: 'static>(
        &mut self,
        component: ComponentType,
    ) -> &Archetype {
        let archetype_id = self.archetype_id_counter;
        let component_vec = Box::new(vec![component]);

        let archetype = Archetype {
            id: archetype_id,
            component_types: vec![component_vec],
            types: vec![TypeId::of::<ComponentType>()],
        };

        self.register_archetype(archetype);

        &self.archetypes[&archetype_id]
    }

    fn move_entity_to_new_archetype(&mut self, entity: EntityId, new_archetype_id: ArchetypeId) {
        let Some(current_record) = self.entity_index.remove(&entity) else {
            return;
        };

        // we remove the elements in order to avoid borrowing issues
        let mut current_archetype = self
            .archetypes
            .remove(&current_record.archetype_id)
            .expect("Internal storage error. Invalid Archetype ID.");
        let mut new_archetype = self
            .archetypes
            .remove(&new_archetype_id)
            .expect("Internal storage error. Invalid Archetype ID.");

        align_and_migrate_archetypes(
            &mut current_archetype,
            &mut new_archetype,
            current_record.entity_row,
        );

        self.archetypes
            .insert(current_archetype.id, current_archetype);
        self.archetypes.insert(new_archetype.id, new_archetype);
    }

    fn register_archetype(&mut self, archetype: Archetype) {
        let archetype_id = archetype.id;

        archetype.types.iter().for_each(|&type_id| {
            self.component_index
                .entry(type_id)
                .or_default()
                .push(archetype_id);
        });

        self.archetypes.insert(archetype_id, archetype);
        self.archetype_id_counter += 1;
    }

    fn remove_archetype(&mut self, archetype_id: ArchetypeId) {
        if self.archetypes.len() == 1 {
            self.archetypes.clear();
            self.component_index.clear();
            self.entity_index.clear();
            return;
        }

        let archetype = self.archetypes.remove(&archetype_id).unwrap();

        archetype.types.iter().for_each(|&type_id| {
            let archetypes = self.component_index.get_mut(&type_id).unwrap();

            if archetypes.len() == 1 {
                self.component_index.remove(&type_id);
            } else {
                archetypes.retain(|&id| id != archetype_id);
            }
        });
    }

    fn has_component<ComponentType: 'static>(&self) -> bool {
        self.component_index
            .contains_key(&TypeId::of::<ComponentType>())
    }

    fn has_entity_component<ComponentType: 'static>(&self, entity: EntityId) -> bool {
        self.get_archetype_for_entity(entity)
            .map_or(false, |archetype| {
                archetype.component_types.iter().any(|column| {
                    column
                        .as_any()
                        .downcast_ref::<Vec<ComponentType>>()
                        .is_some_and(|vec| vec.get(entity).is_some())
                })
            })
    }

    /// Get the archetype for an entity. Returns None if the entity does not exist.
    fn get_archetype_for_entity(&self, entity: EntityId) -> Option<&Archetype> {
        let archetype_id = self.entity_index.get(&entity)?.archetype_id;

        Some(&self.archetypes[&archetype_id])
    }

    pub(crate) fn new() -> Self {
        Self {
            archetypes: HashMap::new(),
            component_index: HashMap::new(),
            entity_index: HashMap::new(),
            archetype_id_counter: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_element_type_id() {
        let component_vec = Box::<Vec<i32>>::default();
        assert_eq!(component_vec.element_type_id(), TypeId::of::<i32>());

        let component_vec = Box::<Vec<f32>>::default();
        assert_eq!(component_vec.element_type_id(), TypeId::of::<f32>());

        let component_vec = Box::<Vec<String>>::default();
        assert_eq!(component_vec.element_type_id(), TypeId::of::<String>());
    }

    #[test]
    fn add_archetype_for_new_component_type_creates_archetype_and_updates_index() {
        let mut storage = Storage::new();

        storage.add_archetype_for_new_component_type(5);
        storage.add_archetype_for_new_component_type(42.0f32);

        assert_eq!(storage.archetypes.len(), 2);
        assert_eq!(storage.component_index.len(), 2);

        let i32_archetypes = storage.component_index.get(&TypeId::of::<i32>());
        assert!(i32_archetypes.is_some());
        let i32_archetypes = i32_archetypes.unwrap();

        let i32_archetype_id = 0;
        assert!(i32_archetypes.contains(&i32_archetype_id));

        let i32_archetype = &storage.archetypes[&i32_archetype_id];
        assert_eq!(i32_archetype.types.len(), 1);
        assert_eq!(i32_archetype.types, vec![TypeId::of::<i32>()]);
        assert_eq!(i32_archetype.component_types.len(), 1);

        let f32_archetypes = storage.component_index.get(&TypeId::of::<f32>());
        assert!(f32_archetypes.is_some());
        let f32_archetypes = f32_archetypes.unwrap();

        let f32_archetype_id = 1;
        assert!(f32_archetypes.contains(&f32_archetype_id));

        let f32_archetype = &storage.archetypes[&f32_archetype_id];
        assert_eq!(f32_archetype.types.len(), 1);
        assert_eq!(f32_archetype.types, vec![TypeId::of::<f32>()]);
        assert_eq!(f32_archetype.component_types.len(), 1);
    }

    #[test]
    fn add_component_to_entity_correctly_creates_archetype_and_updates_index() {
        let mut storage = Storage::new();

        let entity = 0;
        storage.add_component_to_entity(entity, 5);

        assert!(storage.has_component::<i32>());
        assert_eq!(storage.get_archetypes_for_component::<i32>().len(), 1);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);

        let archetype = &storage.archetypes[&0];
        assert_eq!(archetype.types.len(), 1);
        assert_eq!(archetype.component_types.len(), 1);
        assert_eq!(archetype.component_types[0].len(), 1);

        storage.add_component_to_entity(entity, 42.0f32);

        assert!(storage.has_component::<i32>());
        assert!(storage.has_component::<f32>());
        assert_eq!(storage.get_archetypes_for_component::<i32>().len(), 2);
        assert_eq!(storage.get_archetypes_for_component::<f32>().len(), 1);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 2);

        let first_archetype = &storage.archetypes[&0];
        let second_archetype = &storage.archetypes[&1];

        assert_eq!(
            storage.entity_index.get(&entity).unwrap().archetype_id,
            second_archetype.id
        );

        assert_eq!(first_archetype.types.len(), 1);
        assert_eq!(first_archetype.component_types.len(), 1);

        // check if component was migrated to new archetype
        assert_eq!(first_archetype.component_types[0].len(), 0);

        assert_eq!(second_archetype.types.len(), 2);
        assert_eq!(second_archetype.component_types.len(), 2);

        let f32_column = second_archetype
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<f32>>())
            .unwrap();

        assert_eq!(f32_column.len(), 1);

        let i32_column = second_archetype
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<i32>>())
            .unwrap();

        assert_eq!(i32_column.len(), 1);
    }

    #[test]
    fn add_component_to_entity_does_nothing_if_component_already_exists() {
        let mut storage = Storage::new();

        let entity = 0;
        storage.add_component_to_entity(entity, 5);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);

        storage.add_component_to_entity(entity, 5);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);
    }

    #[test]
    fn add_component_to_entity_correctly_updates_different_entities() {
        let mut storage = Storage::new();

        let entity0 = 0;
        storage.add_component_to_entity(entity0, 5);
        storage.add_component_to_entity(entity0, 42.0f32);

        let entity1 = 1;
        storage.add_component_to_entity(entity1, 2);
        storage.add_component_to_entity(entity1, 3.0f32);

        assert_eq!(storage.entity_index.len(), 2);
        assert_eq!(storage.archetypes.len(), 2);
    }

    #[test]
    fn remove_component_from_entity_does_nothing_if_component_does_not_exist() {
        let mut storage = Storage::new();

        let entity = 0;
        storage.add_component_to_entity(entity, 5);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);

        storage.remove_component_from_entity::<f32>(entity, &42.0f32);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);
    }

    #[test]
    fn remove_component_from_entity_correctly_removes_component() {
        let mut storage = Storage::new();

        let entity = 0;
        storage.add_component_to_entity(entity, 5);
        storage.add_component_to_entity(entity, 42.0f32);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 2);

        storage.remove_component_from_entity::<f32>(entity, &42.0f32);

        assert_eq!(storage.entity_index.len(), 1);
        // we don't remove the archetype if it still contains entities, and a standalone f32
        // archetype was not created yet
        assert_eq!(storage.archetypes.len(), 3);

        let archetype = &storage.entity_index.get(&entity).unwrap().archetype_id;
        let archetype = &storage.archetypes[&archetype];
        assert_eq!(archetype.types.len(), 1);
        assert_eq!(archetype.component_types.len(), 1);
        assert_eq!(archetype.component_types[0].len(), 1);
    }

    #[test]
    fn remove_component_from_entity_correctly_creates_archetype_and_updates_index() {
        let mut storage = Storage::new();

        let entity = 0;
        storage.add_component_to_entity(entity, 5);
        storage.add_component_to_entity(entity, 42.0f32);

        storage.add_component_to_entity(1, 5);
        storage.remove_entity(1); // this will remove the i32 standalone archetype

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);

        // we expect this to re-create the archetype for i32
        storage.remove_component_from_entity::<f32>(entity, &42.0f32);

        assert!(storage.has_component::<i32>());
        // we don't remove the archetype even if it's empty
        assert!(storage.has_component::<f32>());
        assert_eq!(storage.get_archetypes_for_component::<i32>().len(), 2);
        assert_eq!(storage.get_archetypes_for_component::<f32>().len(), 1);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 2);

        let archetype = &storage.entity_index.get(&entity).unwrap().archetype_id;
        let archetype = &storage.archetypes[&archetype];
        assert_eq!(archetype.types.len(), 1);
        assert_eq!(archetype.component_types.len(), 1);
        assert_eq!(archetype.component_types[0].len(), 1);

        let component = archetype.get_components::<i32>();
        assert!(component.is_some());
        assert_eq!(component.unwrap().len(), 1);
    }

    #[test]
    fn get_component_vec_returns_correct_component_vec() {
        let archetype = Archetype {
            id: 0,
            component_types: vec![Box::<Vec<i32>>::default()],
            types: vec![TypeId::of::<i32>()],
        };

        let component_vec = archetype.get_components::<i32>();
        assert!(component_vec.is_some());
        assert_eq!(component_vec.unwrap().len(), 0);

        let component_vec = archetype.get_components::<f32>();
        assert!(component_vec.is_none());
    }

    #[test]
    fn get_archetypes_for_component_returns_correct_archetypes() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);

        let archetypes = storage.get_archetypes_for_component::<i32>();
        assert_eq!(archetypes.len(), 1);

        storage.add_component_to_entity(0, 42.0f32);

        let archetypes = storage.get_archetypes_for_component::<i32>();
        assert_eq!(archetypes.len(), 2);
    }

    #[test]
    fn remove_entity_with_single_entity_in_archetype_removes_entity_and_archetype() {
        let mut storage = Storage::new();
        let entity = 0;
        storage.add_component_to_entity(entity, 5);

        storage.remove_entity(entity);

        assert_eq!(storage.entity_index.len(), 0);
        assert_eq!(storage.archetypes.len(), 0);
    }

    #[test]
    fn remove_entity_with_multiple_entities_in_archetype_removes_entity_and_updates_record() {
        let mut storage = Storage::new();

        let entity0 = 0;
        storage.add_component_to_entity(entity0, 5);
        let record_entity0 = storage.entity_index.get(&entity0).unwrap();
        assert_eq!(record_entity0.entity_row, 0);

        let entity1 = 1;
        storage.add_component_to_entity(entity1, 2);
        let record_entity1 = storage.entity_index.get(&entity1).unwrap();
        assert_eq!(record_entity1.entity_row, 1);

        storage.remove_entity(entity0);

        assert_eq!(storage.entity_index.len(), 1);
        assert_eq!(storage.archetypes.len(), 1);

        let archetype = &storage.archetypes[&0];
        assert_eq!(archetype.component_types[0].len(), 1);
        let record_entity1 = storage.entity_index.get(&entity1).unwrap();
        assert_eq!(record_entity1.entity_row, 0);
    }

    #[test]
    fn remove_archetype_empties_component_index() {
        let mut storage = Storage::new();
        let archetype_id = storage.add_archetype_for_new_component_type(5).id;

        storage.remove_archetype(archetype_id);

        assert_eq!(storage.component_index.len(), 0);
    }

    #[test]
    fn remove_archetype_updates_component_index_for_type() {
        let mut storage = Storage::new();
        // this creates the [i32] archetype with id 0
        storage.add_component_to_entity(0, 5);

        storage.add_component_to_entity(1, 2);
        storage.add_component_to_entity(1, 3.0f32);

        assert_eq!(
            storage
                .get_archetype_ids_for_component::<i32>()
                .unwrap()
                .len(),
            2
        );

        assert_eq!(
            storage
                .get_archetype_ids_for_component::<f32>()
                .unwrap()
                .len(),
            1
        );

        storage.remove_archetype(0);

        assert_eq!(storage.component_index.len(), 2);
        assert_eq!(
            storage
                .component_index
                .get(&TypeId::of::<i32>())
                .unwrap()
                .get(0),
            Some(1).as_ref()
        );
        assert_eq!(storage.get_archetypes_for_component::<i32>().len(), 1);
        assert_eq!(storage.get_archetypes_for_component::<f32>().len(), 1);

        storage.remove_archetype(1);

        assert_eq!(storage.component_index.len(), 0);
        assert_eq!(storage.get_archetypes_for_component::<i32>().len(), 0);
        assert_eq!(storage.get_archetypes_for_component::<f32>().len(), 0);
    }
}
