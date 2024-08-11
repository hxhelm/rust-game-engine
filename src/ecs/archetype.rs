use std::any::TypeId;
use crate::ecs::storage::{ComponentVec, EntityRow};

#[allow(clippy::module_name_repetitions)]
pub type ArchetypeId = usize;

pub struct Archetype {
    pub(crate) id: ArchetypeId,
    pub(crate) component_types: Vec<Box<dyn ComponentVec>>,
    pub(crate) types: Vec<TypeId>,
}

impl Archetype {
    pub(crate) fn new_from_add<ComponentType: 'static>(from_archetype: &Self, id: usize) -> Self {
        let mut component_types: Vec<Box<dyn ComponentVec>> = from_archetype
            .component_types
            .iter()
            .map(|column| column.new_empty())
            .collect();

        // We allow a panic, since if this fails, then we have a bug in the ECS design.
        assert!(!component_types
            .iter()
            .any(|component_type| component_type.as_any().is::<Vec<ComponentType>>()));

        component_types.push(Box::<Vec<ComponentType>>::default());

        // when adding new components, we want to keep the order of the types consistent
        component_types.sort_by_key(|a| a.element_type_id());

        // for now, the order of the types vector does not matter, this might change in the future
        // if we chose to look up the component types by type id
        let mut types: Vec<_> = from_archetype
            .types
            .iter()
            .chain(std::iter::once(&TypeId::of::<ComponentType>()))
            .copied()
            .collect();

        types.sort();

        Self {
            id,
            component_types,
            types,
        }
    }

    pub(crate) fn new_from_remove<ComponentType: 'static>(
        from_archetype: &Self,
        id: usize,
    ) -> Self {
        let mut component_types: Vec<Box<dyn ComponentVec>> = from_archetype
            .component_types
            .iter()
            .map(|column| column.new_empty())
            .collect();

        // We allow a panic, since if this fails, then we have a bug in the ECS design.
        let target_type_index = component_types
            .iter()
            .position(|component_type| component_type.as_any().is::<Vec<ComponentType>>())
            .expect("Component type not found.");

        component_types.remove(target_type_index);

        let mut types = from_archetype.types.clone();
        types.remove(target_type_index);

        Self {
            id,
            component_types,
            types,
        }
    }

    #[cfg(test)]
    pub(crate) fn get_components<ComponentType: 'static>(&self) -> Option<&[ComponentType]> {
        self.component_types.iter().find_map(|column| {
            column
                .as_any()
                .downcast_ref::<Vec<ComponentType>>()
                .map(Vec::as_slice)
        })
    }

    fn get_components_mut<ComponentType: 'static>(&mut self) -> Option<&mut Vec<ComponentType>> {
        self.component_types
            .iter_mut()
            .find_map(|column| column.as_any_mut().downcast_mut::<Vec<ComponentType>>())
    }

    pub(crate) fn push_component<ComponentType: 'static>(&mut self, component: ComponentType) {
        let column: &mut Vec<ComponentType> = self
            .get_components_mut()
            .expect("Component type not found.");

        column.push(component);
    }
}

/// Aligns two archetypes and migrates the components of the source archetype to the target
/// archetype. This is used when moving an entity to a new archetype.
/// Note: This only migrates the components that are shared between the two archetypes.
pub fn align_and_migrate_archetypes(
    source: &mut Archetype,
    target: &mut Archetype,
    source_entity_row: EntityRow,
) {
    let mut i = 0;
    let mut j = 0;

    while i < source.types.len() && j < target.types.len() {
        match source.types[i].cmp(&target.types[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let col_source = &mut source.component_types[i];
                let col_target = &mut target.component_types[j];

                if col_source.len() > source_entity_row {
                    col_source.migrate_element(source_entity_row, &mut **col_target);
                }

                i += 1;
                j += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_from_add_sets_correct_data() {
        let archetype = Archetype {
            id: 0,
            component_types: vec![Box::<Vec<i32>>::default()],
            types: vec![TypeId::of::<i32>()],
        };

        let new_archetype = Archetype::new_from_add::<f32>(&archetype, 1);

        let mut type_ids_sorted = vec![TypeId::of::<i32>(), TypeId::of::<f32>()];

        type_ids_sorted.sort();

        assert_eq!(new_archetype.types.len(), 2);
        assert_eq!(new_archetype.types, type_ids_sorted);
        assert_eq!(new_archetype.component_types.len(), 2);

        assert_eq!(
            new_archetype.component_types[0].element_type_id(),
            type_ids_sorted[0]
        );
        assert_eq!(
            new_archetype.component_types[1].element_type_id(),
            type_ids_sorted[1]
        );
    }

    #[test]
    fn new_from_remove_sets_correct_data() {
        let archetype = Archetype {
            id: 0,
            component_types: vec![Box::<Vec<i32>>::default(), Box::<Vec<f32>>::default()],
            types: vec![TypeId::of::<i32>(), TypeId::of::<f32>()],
        };

        let new_archetype = Archetype::new_from_remove::<f32>(&archetype, 1);

        assert_eq!(new_archetype.types.len(), 1);
        assert_eq!(new_archetype.types, vec![TypeId::of::<i32>()]);
        assert_eq!(new_archetype.component_types.len(), 1);
        assert!(new_archetype.component_types[0].is_empty());

        assert_eq!(
            new_archetype.component_types[0].element_type_id(),
            TypeId::of::<i32>()
        );
    }

    #[test]
    fn align_and_migrate_archetypes_correctly_migrates_archetypes() {
        let mut source = Archetype {
            id: 0,
            component_types: vec![Box::new(vec![1, 2, 3])],
            types: vec![TypeId::of::<i32>()],
        };

        let mut target = Archetype {
            id: 1,
            component_types: vec![
                Box::new(vec![1.0_f32, 2.0_f32, 3.0_f32]),
                Box::new(vec![1, 2, 3]),
            ],
            types: vec![TypeId::of::<f32>(), TypeId::of::<i32>()],
        };

        target.types.sort();
        target.component_types.sort_by_key(|a| a.element_type_id());

        align_and_migrate_archetypes(&mut source, &mut target, 1);

        let source_i32_components = source.component_types[0]
            .as_any()
            .downcast_ref::<Vec<i32>>()
            .unwrap();

        let target_f32_components = target
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<f32>>())
            .map(|column| column.as_any().downcast_ref::<Vec<f32>>().unwrap())
            .unwrap();

        let target_i32_components = target
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<i32>>())
            .map(|column| column.as_any().downcast_ref::<Vec<i32>>().unwrap())
            .unwrap();

        assert_eq!(source_i32_components, &vec![1, 3]);
        assert_eq!(target_f32_components, &vec![1.0_f32, 2.0_f32, 3.0_f32]);
        assert_eq!(target_i32_components, &vec![1, 2, 3, 2]);
    }

    #[test]
    fn align_and_migrate_archetypes_merges_into_empty_archetype() {
        let mut source = Archetype {
            id: 0,
            component_types: vec![
                Box::new(vec![1.0_f32, 2.0_f32, 3.0_f32]),
                Box::new(vec![1, 2, 3]),
            ],
            types: vec![TypeId::of::<f32>(), TypeId::of::<i32>()],
        };

        let mut target = Archetype {
            id: 1,
            component_types: vec![Box::new(Vec::<i32>::new())],
            types: vec![TypeId::of::<i32>()],
        };

        align_and_migrate_archetypes(&mut source, &mut target, 1);

        let source_i32_components = source
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<i32>>())
            .map(|column| column.as_any().downcast_ref::<Vec<i32>>().unwrap())
            .unwrap();

        let source_f32_components = source
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<f32>>())
            .map(|column| column.as_any().downcast_ref::<Vec<f32>>().unwrap())
            .unwrap();

        let target_i32_components = target
            .component_types
            .iter()
            .find(|column| column.as_any().is::<Vec<i32>>())
            .map(|column| column.as_any().downcast_ref::<Vec<i32>>().unwrap())
            .unwrap();

        assert_eq!(source_i32_components, &vec![1, 3]);
        assert_eq!(source_f32_components, &vec![1.0_f32, 2.0_f32, 3.0_f32]);
        assert_eq!(target_i32_components, &vec![2]);
    }
}
