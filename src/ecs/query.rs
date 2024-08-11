use super::archetype::{Archetype, ArchetypeId};
use crate::ecs::storage::ComponentVec;
use crate::ecs::Storage;
use itertools::{izip, Itertools};
use std::any::TypeId;
use std::collections::HashSet;

const MESSAGE_DUPLICATE_COMPONENT_TYPE: &str =
    "Component types must be different when querying more than one component type";

/// The `Query` trait provides methods to iterate over a collection of components.
///
/// # Examples
/// ```
/// use game_engine::ecs::{World, Query};
///
/// let mut world = World::init().unwrap();
///
/// world.build_entity()
///     .with_component(42)
///     .build();
///
/// for transform in world.storage.query_one::<i32>() {
///     assert!(transform.eq(&42));
/// }
/// ```
///
/// ```
/// use game_engine::ecs::{World, Query};
///
/// let mut world = World::init().unwrap();
///
/// world.build_entity()
///     .with_component(42)
///     .with_component(24.0f32)
///     .build();
///
/// for (int_component, float_component) in world.storage.query_two::<i32, f32>() {
///     assert!(int_component.eq(&42));
///     assert!(float_component.eq(&24.0f32));
/// }
/// ```
///
/// # Panics
///
/// Panics if two component types are the same.
pub trait Query {
    fn query_one<ComponentType: 'static>(&self) -> impl Iterator<Item = &ComponentType>;
    fn query_one_mut<ComponentType: 'static>(&mut self)
        -> impl Iterator<Item = &mut ComponentType>;
    fn query_two<ComponentType1: 'static, ComponentType2: 'static>(
        &self,
    ) -> impl Iterator<Item = (&ComponentType1, &ComponentType2)>;
    fn query_two_mut<ComponentType1: 'static, ComponentType2: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (&mut ComponentType1, &mut ComponentType2)>;
    fn query_three<ComponentType1: 'static, ComponentType2: 'static, ComponentType3: 'static>(
        &self,
    ) -> impl Iterator<Item = (&ComponentType1, &ComponentType2, &ComponentType3)>;
    fn query_three_mut<ComponentType1: 'static, ComponentType2: 'static, ComponentType3: 'static>(
        &mut self,
    ) -> impl Iterator<
        Item = (
            &mut ComponentType1,
            &mut ComponentType2,
            &mut ComponentType3,
        ),
    >;
    fn query_four<
        ComponentType1: 'static,
        ComponentType2: 'static,
        ComponentType3: 'static,
        ComponentType4: 'static,
    >(
        &self,
    ) -> impl Iterator<
        Item = (
            &ComponentType1,
            &ComponentType2,
            &ComponentType3,
            &ComponentType4,
        ),
    >;
    fn query_four_mut<
        ComponentType1: 'static,
        ComponentType2: 'static,
        ComponentType3: 'static,
        ComponentType4: 'static,
    >(
        &mut self,
    ) -> impl Iterator<
        Item = (
            &mut ComponentType1,
            &mut ComponentType2,
            &mut ComponentType3,
            &mut ComponentType4,
        ),
    >;
}

macro_rules! iterate_components_base {
    ($storage:ident, $($component:ty),*; $get_archetypes:ident, $iter_components:ident, $as_any_fn:ident, $downcast_fn:ident) => {{
        use itertools::izip;
        use std::any::TypeId;
        use std::collections::HashSet;

        let type_ids = vec![
            $(TypeId::of::<$component>(),)*
        ];

        assert_eq!(
            type_ids.iter().collect::<HashSet<_>>().len(),
            type_ids.len(),
            "{MESSAGE_DUPLICATE_COMPONENT_TYPE}"
        );

        let common_archetype_ids = get_archetype_ids_for_types($storage, &type_ids);
        let archetypes = $get_archetypes($storage, &common_archetype_ids);

        archetypes.into_iter().flat_map(move |archetype| {
            let mut components = $iter_components(archetype, &type_ids);

            izip!(
                $(
                    components
                        .next()
                        .unwrap()
                        .$as_any_fn()
                        .$downcast_fn::<Vec<$component>>()
                        .unwrap(),
                )*
            )
        })
    }};
}

macro_rules! iterate_components {
    ($storage:ident, $($component:ty),*) => {
        iterate_components_base!($storage, $($component),*; get_archetypes_by_ids, iter_archetype_components_by_type_ids, as_any, downcast_ref)
    };
}

macro_rules! iterate_components_mut {
    ($storage:ident, $($component:ty),*) => {
        iterate_components_base!($storage, $($component),*; get_archetypes_by_ids_mut, iter_mut_archetype_components_by_type_ids, as_any_mut, downcast_mut)
    };
}

impl Query for Storage {
    fn query_one<ComponentType: 'static>(&self) -> impl Iterator<Item = &ComponentType> {
        self.get_archetypes_for_component::<ComponentType>()
            .into_iter()
            .flat_map(iter_archetype_components_unchecked::<ComponentType>)
    }

    fn query_one_mut<ComponentType: 'static>(
        &mut self,
    ) -> impl Iterator<Item = &mut ComponentType> {
        self.get_archetypes_for_component_mut::<ComponentType>()
            .into_iter()
            .flat_map(iter_mut_archetype_components_unchecked::<ComponentType>)
    }

    fn query_two<ComponentType1: 'static, ComponentType2: 'static>(
        &self,
    ) -> impl Iterator<Item = (&ComponentType1, &ComponentType2)> {
        iterate_components!(self, ComponentType1, ComponentType2)
    }

    fn query_two_mut<ComponentType1: 'static, ComponentType2: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (&mut ComponentType1, &mut ComponentType2)> {
        iterate_components_mut!(self, ComponentType1, ComponentType2)
    }

    fn query_three<ComponentType1: 'static, ComponentType2: 'static, ComponentType3: 'static>(
        &self,
    ) -> impl Iterator<Item = (&ComponentType1, &ComponentType2, &ComponentType3)> {
        iterate_components!(self, ComponentType1, ComponentType2, ComponentType3)
    }

    fn query_three_mut<
        ComponentType1: 'static,
        ComponentType2: 'static,
        ComponentType3: 'static,
    >(
        &mut self,
    ) -> impl Iterator<
        Item = (
            &mut ComponentType1,
            &mut ComponentType2,
            &mut ComponentType3,
        ),
    > {
        iterate_components_mut!(self, ComponentType1, ComponentType2, ComponentType3)
    }
    fn query_four<
        ComponentType1: 'static,
        ComponentType2: 'static,
        ComponentType3: 'static,
        ComponentType4: 'static,
    >(
        &self,
    ) -> impl Iterator<
        Item = (
            &ComponentType1,
            &ComponentType2,
            &ComponentType3,
            &ComponentType4,
        ),
    > {
        iterate_components!(
            self,
            ComponentType1,
            ComponentType2,
            ComponentType3,
            ComponentType4
        )
    }

    fn query_four_mut<
        ComponentType1: 'static,
        ComponentType2: 'static,
        ComponentType3: 'static,
        ComponentType4: 'static,
    >(
        &mut self,
    ) -> impl Iterator<
        Item = (
            &mut ComponentType1,
            &mut ComponentType2,
            &mut ComponentType3,
            &mut ComponentType4,
        ),
    > {
        iterate_components_mut!(
            self,
            ComponentType1,
            ComponentType2,
            ComponentType3,
            ComponentType4
        )
    }
}

fn get_archetype_ids_for_types(storage: &Storage, type_ids: &[TypeId]) -> Vec<ArchetypeId> {
    let mut archetype_sets: Vec<HashSet<_>> = type_ids
        .iter()
        .map(|type_id| {
            let archetypes = storage.component_index.get(type_id);

            archetypes.map_or_else(HashSet::new, |archetypes| {
                archetypes.iter().copied().collect::<HashSet<_>>()
            })
        })
        .collect();

    // get the smallest set
    let smallest_set_pos = archetype_sets
        .iter()
        .position_min_by_key(|set| set.len())
        .unwrap();
    let mut smallest_set = archetype_sets.swap_remove(smallest_set_pos);

    // get the intersection of the sets
    for set in archetype_sets {
        smallest_set = smallest_set.intersection(&set).copied().collect();
    }

    smallest_set.iter().copied().collect()
}

fn get_archetypes_by_ids<'a>(storage: &'a Storage, ids: &[ArchetypeId]) -> Vec<&'a Archetype> {
    ids.iter()
        .map(|id| storage.archetypes.get(id).expect("Archetype not found."))
        .collect()
}

fn get_archetypes_by_ids_mut<'a>(
    storage: &'a mut Storage,
    ids: &[ArchetypeId],
) -> Vec<&'a mut Archetype> {
    storage
        .archetypes
        .iter_mut()
        .filter(|(id, _)| ids.contains(id))
        .map(|(_, a)| a)
        .collect()
}

fn iter_mut_archetype_components_unchecked<ComponentType: 'static>(
    archetype: &mut Archetype,
) -> impl Iterator<Item = &mut ComponentType> {
    archetype
        .component_types
        .iter_mut()
        .find_map(|column| column.as_any_mut().downcast_mut::<Vec<ComponentType>>())
        .expect("Component type not found.")
        .iter_mut()
}

fn iter_archetype_components_unchecked<ComponentType: 'static>(
    archetype: &Archetype,
) -> impl Iterator<Item = &ComponentType> {
    archetype
        .component_types
        .iter()
        .find_map(|column| column.as_any().downcast_ref::<Vec<ComponentType>>())
        .expect("Component type not found.")
        .iter()
}

fn iter_archetype_components_by_type_ids<'a>(
    archetype: &'a Archetype,
    type_ids: &[TypeId],
) -> impl Iterator<Item = &'a Box<dyn ComponentVec>> {
    archetype
        .component_types
        .iter()
        .filter(|column| type_ids.contains(&column.element_type_id()))
        .sorted_by_key(|column| {
            type_ids
                .iter()
                .position(|&id| id == column.element_type_id())
        })
}

fn iter_mut_archetype_components_by_type_ids<'a>(
    archetype: &'a mut Archetype,
    type_ids: &[TypeId],
) -> impl Iterator<Item = &'a mut Box<dyn ComponentVec>> {
    archetype
        .component_types
        .iter_mut()
        .filter(|column| type_ids.contains(&column.element_type_id()))
        .sorted_by_key(|column| {
            type_ids
                .iter()
                .position(|&id| id == column.element_type_id())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_one_returns_correct_iterator() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(0, 42.0f32);

        storage.add_component_to_entity(1, 5);

        let mut iterator = storage.query_one::<f32>();
        let first = iterator.next();

        assert!(first.is_some());
        assert!(first.unwrap().eq(&42.0f32));
        assert!(iterator.next().is_none());

        let iterator = storage.query_one::<i32>();

        for i in iterator {
            assert!(i.eq(&5));
        }
    }

    #[test]
    fn query_one_returns_empty_iterator_when_no_components_match() {
        let storage = Storage::new();
        let mut iterator = storage.query_one::<i32>();
        assert!(iterator.next().is_none());
    }

    #[test]
    fn query_one_mut_returns_correct_iterator() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(0, 42.0f32);

        let mut iterator = storage.query_one_mut::<f32>();
        let first = iterator.next();

        assert!(first.is_some());
        assert_eq!(first.unwrap(), &mut 42.0f32);
        assert!(iterator.next().is_none());
    }

    #[test]
    fn query_one_mut_returns_empty_iterator_when_no_components_match() {
        let mut storage = Storage::new();
        let mut iterator = storage.query_one_mut::<i32>();
        assert!(iterator.next().is_none());
    }

    #[test]
    fn query_two_returns_correct_iterator() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(0, 42.0f32);
        storage.add_component_to_entity(1, 5);
        storage.add_component_to_entity(1, 24.0f32);

        let mut iterator = storage.query_two::<i32, f32>();
        let first = iterator.next();

        assert!(first.is_some());
        let (int_component, float_component) = first.unwrap();
        assert!(int_component.eq(&5));
        assert!(float_component.eq(&42.0f32));

        let second = iterator.next();
        assert!(second.is_some());
        let (int_component, float_component) = second.unwrap();
        assert!(int_component.eq(&5));
        assert!(float_component.eq(&24.0f32));

        assert!(iterator.next().is_none());
    }

    #[test]
    fn query_two_returns_empty_iterator_when_no_common_components_match() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(1, 42.0f32);

        let mut iterator = storage.query_two::<i32, f32>();
        assert!(iterator.next().is_none());
    }

    #[test]
    fn query_two_mut_returns_correct_iterator() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(0, 42.0f32);
        storage.add_component_to_entity(1, 5);
        storage.add_component_to_entity(1, 42.0f32);

        let iterator = storage.query_two_mut::<i32, f32>();

        let mut count = 0;
        for (int_component, float_component) in iterator {
            assert_eq!(int_component, &mut 5);
            assert_eq!(float_component, &mut 42.0f32);

            *int_component = 10;
            *float_component = 24.0f32;
            count += 1;
        }

        assert_eq!(count, 2);

        let iterator = storage.query_two_mut::<f32, i32>();

        for (float_component, int_component) in iterator {
            assert_eq!(float_component, &mut 24.0f32);
            assert_eq!(int_component, &mut 10);
        }
    }

    #[test]
    fn query_two_mut_returns_empty_iterator_when_no_common_components_match() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(1, 42.0f32);

        let mut iterator = storage.query_two_mut::<i32, f32>();
        assert!(iterator.next().is_none());
    }

    #[test]
    fn query_three_mut_returns_correct_iterator() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(0, 42.0f32);
        storage.add_component_to_entity(0, b'a');
        storage.add_component_to_entity(1, 5);
        storage.add_component_to_entity(1, 42.0f32);
        storage.add_component_to_entity(1, b'a');

        let iterator = storage.query_three_mut::<i32, f32, u8>();

        let mut count = 0;
        for (int_component, float_component, byte_component) in iterator {
            assert_eq!(int_component, &mut 5);
            assert_eq!(float_component, &mut 42.0f32);
            assert_eq!(byte_component, &mut b'a');

            *int_component = 10;
            *float_component = 24.0f32;
            *byte_component = b'b';
            count += 1;
        }

        assert_eq!(count, 2);

        let iterator = storage.query_three_mut::<f32, u8, i32>();

        for (float_component, byte_component, int_component) in iterator {
            assert_eq!(float_component, &mut 24.0f32);
            assert_eq!(byte_component, &mut b'b');
            assert_eq!(int_component, &mut 10);
        }
    }

    #[test]
    fn query_three_mut_returns_empty_iterator_when_no_common_components_match() {
        let mut storage = Storage::new();
        storage.add_component_to_entity(0, 5);
        storage.add_component_to_entity(1, 42.0f32);

        let mut iterator = storage.query_three_mut::<i32, f32, u8>();
        assert!(iterator.next().is_none());
    }
}
