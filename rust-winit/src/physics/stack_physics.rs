//! The stack physics world, corresponding declaration-for-declaration with `stackPhysics.ts`.

pub const PHYSICS_GRAVITY: f32 = 10.8;
pub const PHYSICS_STEP: f64 = 1.0 / 60.0;
pub const FINAL_SETTLE_SECONDS: f64 = 2.35;

/// Ground heights across the pasture, in physics coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundProfile {
    pub min_x: f64,
    pub step: f64,
    pub heights: Vec<f64>,
}

/// Interpolates the modelled ground, with the flat pasture as the same fallback used by the world.
pub fn get_ground_y(ground: Option<&GroundProfile>, x: f64) -> f64 {
    let Some(ground) = ground.filter(|profile| !profile.heights.is_empty()) else {
        return super::PASTURE_TOP_Y;
    };
    let position = (x - ground.min_x) / ground.step;
    let last = ground.heights.len() - 1;
    if position <= 0.0 {
        return ground.heights[0];
    }
    if position >= last as f64 {
        return ground.heights[last];
    }
    let index = position.floor() as usize;
    let fraction = position - index as f64;
    ground.heights[index] + (ground.heights[index + 1] - ground.heights[index]) * fraction
}

#[cfg(feature = "flight-physics")]
mod flight {
    use std::collections::HashSet;

    use flight_sdk::{
        CollisionAabb2D, CollisionCircle2D, CollisionPolygon2D, CollisionRegistry2D,
        CollisionShape2D, Physics2DBodyType, Physics2DMaterial, Physics2DWorld, Ref, RigidBody2D,
        add_physics2d_body, create_collision_manifold2d, create_physics2d_collider,
        create_physics2d_world, create_rigid_body2d, create_uniform_grid_spatial_backend_2d,
        register_built_in_collision_pair_tests2d, register_built_in_collision_supports2d,
        remove_physics2d_body, step_physics2d, test_collision2d,
        update_physics2d_collider_world_shape,
    };

    use super::{GroundProfile, PHYSICS_GRAVITY, PHYSICS_STEP, get_ground_y};
    use crate::physics::{
        METERS_PER_WORLD_UNIT, PASTURE_MAX_X, PASTURE_MIN_X, PASTURE_TOP_Y, StackBodyRegistry,
        StackObjectKind, delete_stack_body, delete_stack_body_anchor, find_stack_body_kind,
        get_centred_collider_polygons, get_stack_body_anchor, get_stack_body_half_width,
        get_stack_body_inertia, get_stack_body_kind, get_stack_body_quiet,
        get_stack_body_vertical_extent, get_stack_object_profile, set_stack_body_anchor,
        set_stack_body_inertia, set_stack_body_kind, set_stack_body_quiet,
    };

    const PHYSICS_GRID_CELL_SIZE: f32 = 0.2;
    const GROUND_SKIRT: f64 = 0.35;
    const STILL_LINEAR: f32 = 0.35;
    const STILL_ANGULAR: f32 = 1.1;
    const SETTLE_DELAY: f64 = 0.5;
    const SETTLE_RAMP: f64 = 1.6;
    const ASSIST_ANGULAR: f32 = 16.5;
    const ASSIST_LINEAR: f32 = 3.3;
    const ASSIST_INERTIA: f32 = 10.0;
    const COHESION_PULL: f32 = 78.0;
    const COHESION_GRIP: f32 = 21.0;
    const COHESION_DEADZONE: f32 = 0.0016;
    const STABILITY_FULL_HEIGHT: f32 = 0.9;

    const PASTURE_MATERIAL: Physics2DMaterial = Physics2DMaterial {
        density: 0.0,
        friction: 0.38,
        restitution: 0.035,
    };

    #[derive(Clone, Copy)]
    struct StackDamping {
        angular: f32,
        linear: f32,
    }

    /// Rust keeps Flight's caller-owned collision registry and the TS side tables beside the world.
    pub struct HorseStackWorld {
        pub physics: Physics2DWorld,
        collision_registry: CollisionRegistry2D,
        pub body_registry: StackBodyRegistry,
    }

    /// Builds the round's physics world, including the flat or terrain-following pasture floor.
    pub fn create_horse_stack_world(ground: Option<&GroundProfile>) -> HorseStackWorld {
        let mut physics = create_physics2d_world();
        physics.gravity_x = 0.0;
        physics.gravity_y = -PHYSICS_GRAVITY;
        physics.index = create_uniform_grid_spatial_backend_2d(PHYSICS_GRID_CELL_SIZE);
        physics.config.velocity_iterations = 18;
        physics.config.position_iterations = 9;
        physics.config.time_to_sleep = 0.3;
        physics.config.sleep_linear_threshold = 0.01 * METERS_PER_WORLD_UNIT as f32;
        physics.config.penetration_slop = 0.009;
        physics.config.position_correction = 0.28;
        physics.config.restitution_threshold = 2.5;

        if ground.is_none_or(|profile| profile.heights.len() < 2) {
            let pasture = create_body(Physics2DBodyType::Static, 0.0, PASTURE_TOP_Y - 0.02, 0.0);
            pasture.write().unwrap().colliders.push(create_collider(
                CollisionShape2D::Aabb(CollisionAabb2D {
                    min_x: PASTURE_MIN_X as f32,
                    min_y: -0.02,
                    max_x: PASTURE_MAX_X as f32,
                    max_y: 0.02,
                }),
                PASTURE_MATERIAL,
            ));
            add_physics2d_body(&mut physics, &pasture);
        } else if let Some(ground) = ground {
            let terrain = create_body(Physics2DBodyType::Static, 0.0, 0.0, 0.0);
            for (index, heights) in ground.heights.windows(2).enumerate() {
                let x0 = ground.min_x + ground.step * index as f64;
                let x1 = x0 + ground.step;
                let y0 = heights[0];
                let y1 = heights[1];
                let floor = y0.min(y1) - GROUND_SKIRT;
                terrain.write().unwrap().colliders.push(create_collider(
                    CollisionShape2D::Polygon(CollisionPolygon2D {
                        points: vec![x0, floor, x1, floor, x1, y1, x0, y0],
                    }),
                    PASTURE_MATERIAL,
                ));
            }
            add_physics2d_body(&mut physics, &terrain);
        }

        HorseStackWorld {
            physics,
            collision_registry: create_collision_registry(),
            body_registry: StackBodyRegistry::default(),
        }
    }

    /// Would this held piece overlap any placed body, using the same shapes as the solver?
    pub fn does_stack_placement_overlap(
        kind: StackObjectKind,
        x: f64,
        y: f64,
        angle: f64,
        bodies: &[Ref<RigidBody2D>],
    ) -> bool {
        let collision_registry = create_collision_registry();
        let probe = create_stack_object_body(kind, x, y, angle);
        update_body_world_shapes(&probe);
        let probe = probe.read().unwrap();
        let mut manifold = create_collision_manifold2d();

        for body in bodies {
            let body = body.read().unwrap();
            for probe_collider in &probe.colliders {
                let probe_collider = probe_collider.read().unwrap();
                for collider in &body.colliders {
                    let collider = collider.read().unwrap();
                    if test_collision2d(
                        &collision_registry,
                        &probe_collider.world,
                        &collider.world,
                        &mut manifold,
                    ) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn add_stack_object_body(
        world: &mut HorseStackWorld,
        kind: StackObjectKind,
        x: f64,
        y: f64,
        angle: f64,
    ) -> Ref<RigidBody2D> {
        let body = create_stack_object_body(kind, x, y, angle);
        {
            let damping = stack_damping(kind);
            let mut body = body.write().unwrap();
            body.linear_damping = damping.linear;
            body.angular_damping = damping.angular;
            body.bullet = false;
        }
        add_physics2d_body(&mut world.physics, &body);
        {
            let body = body.read().unwrap();
            set_stack_body_kind(&mut world.body_registry, &body, kind);
        }
        update_body_world_shapes(&body);
        body
    }

    pub fn step_horse_stack(world: &mut HorseStackWorld) {
        apply_stack_assist(world);
        step_physics2d(
            &mut world.physics,
            PHYSICS_STEP,
            &world.collision_registry,
            None,
        );
    }

    pub fn remove_stack_object_body(world: &mut HorseStackWorld, body: &Ref<RigidBody2D>) -> bool {
        {
            let body = body.read().unwrap();
            delete_stack_body(&mut world.body_registry, &body);
        }
        remove_physics2d_body(&mut world.physics, body)
    }

    pub const STACK_CONTACT_GROUND: u8 = 1;
    pub const STACK_CONTACT_PIECE: u8 = 2;

    pub fn get_stack_body_contacts(world: &HorseStackWorld, body: &Ref<RigidBody2D>) -> u8 {
        let body_index = body.read().unwrap().index;
        let mut contacts = 0;
        for contact in &world.physics.contacts {
            let contact = contact.read().unwrap();
            if !contact.touching || contact.sensor {
                continue;
            }
            let other = if contact.body_a == body_index {
                contact.body_b
            } else if contact.body_b == body_index {
                contact.body_a
            } else {
                continue;
            };
            let Some(other_body) = world.physics.body_by_index.get(&other) else {
                continue;
            };
            contacts |= if find_stack_body_kind(&world.body_registry, &other_body.read().unwrap())
                .is_some()
            {
                STACK_CONTACT_PIECE
            } else {
                STACK_CONTACT_GROUND
            };
            if contacts == STACK_CONTACT_GROUND | STACK_CONTACT_PIECE {
                break;
            }
        }
        contacts
    }

    pub fn is_stack_body_within_pasture(registry: &StackBodyRegistry, body: &RigidBody2D) -> bool {
        let reach = get_stack_body_half_width(registry, body);
        f64::from(body.x) >= PASTURE_MIN_X - reach && f64::from(body.x) <= PASTURE_MAX_X + reach
    }

    pub fn get_supported_stack_height(
        world: &HorseStackWorld,
        objects: &[Ref<RigidBody2D>],
        ground: Option<&GroundProfile>,
    ) -> f64 {
        let mut touching_bodies = HashSet::new();
        for contact in &world.physics.contacts {
            let contact = contact.read().unwrap();
            if contact.touching && !contact.sensor {
                touching_bodies.insert(contact.body_a);
                touching_bodies.insert(contact.body_b);
            }
        }

        let mut height: f64 = 0.0;
        for body in objects {
            let body = body.read().unwrap();
            let on_stack = body.sleeping || touching_bodies.contains(&body.index);
            if !on_stack || !is_stack_body_on_stack(&world.body_registry, &body, ground) {
                continue;
            }
            height = height.max(
                f64::from(body.y) + get_stack_body_vertical_extent(&world.body_registry, &body),
            );
        }
        height
    }

    pub fn get_settled_stack_height(
        world: &HorseStackWorld,
        objects: &[Ref<RigidBody2D>],
        ground: Option<&GroundProfile>,
    ) -> f64 {
        let mut height: f64 = 0.0;
        for body in objects {
            let body = body.read().unwrap();
            if !body.sleeping || !is_stack_body_on_stack(&world.body_registry, &body, ground) {
                continue;
            }
            height = height.max(
                f64::from(body.y) + get_stack_body_vertical_extent(&world.body_registry, &body),
            );
        }
        height
    }

    fn create_collision_registry() -> CollisionRegistry2D {
        let mut registry = CollisionRegistry2D::default();
        register_built_in_collision_supports2d(&mut registry);
        register_built_in_collision_pair_tests2d(&mut registry);
        registry
    }

    fn create_body(body_type: Physics2DBodyType, x: f64, y: f64, angle: f64) -> Ref<RigidBody2D> {
        let body = create_rigid_body2d();
        {
            let mut body = body.write().unwrap();
            body.body_type = body_type;
            body.x = x as f32;
            body.y = y as f32;
            body.angle = angle as f32;
        }
        body
    }

    fn create_collider(
        shape: CollisionShape2D,
        material: Physics2DMaterial,
    ) -> Ref<flight_sdk::Physics2DCollider> {
        let collider = create_physics2d_collider(&shape);
        collider.write().unwrap().material = material;
        collider
    }

    fn create_stack_object_body(
        kind: StackObjectKind,
        x: f64,
        y: f64,
        angle: f64,
    ) -> Ref<RigidBody2D> {
        let body = create_body(Physics2DBodyType::Dynamic, x, y, angle);
        if kind == StackObjectKind::Chickens {
            let radius = get_stack_object_profile(StackObjectKind::Chickens).half_height;
            body.write().unwrap().colliders.push(create_collider(
                CollisionShape2D::Circle(CollisionCircle2D {
                    x: 0.0,
                    y: 0.0,
                    radius,
                }),
                stack_material(kind),
            ));
            return body;
        }
        for points in get_centred_collider_polygons(kind).expect("non-circular stack object") {
            body.write().unwrap().colliders.push(create_collider(
                CollisionShape2D::Polygon(CollisionPolygon2D { points }),
                stack_material(kind),
            ));
        }
        body
    }

    fn update_body_world_shapes(body: &Ref<RigidBody2D>) {
        let body = body.read().unwrap();
        for collider in &body.colliders {
            update_physics2d_collider_world_shape(&mut collider.write().unwrap(), &body);
        }
    }

    fn stack_material(kind: StackObjectKind) -> Physics2DMaterial {
        match kind {
            StackObjectKind::Horse => Physics2DMaterial {
                density: 1.0,
                friction: 0.82,
                restitution: 0.01,
            },
            StackObjectKind::Hay => Physics2DMaterial {
                density: 0.72,
                friction: 0.9,
                restitution: 0.0,
            },
            StackObjectKind::Cow => Physics2DMaterial {
                density: 1.15,
                friction: 0.84,
                restitution: 0.01,
            },
            StackObjectKind::Chickens => Physics2DMaterial {
                density: 0.48,
                friction: 0.72,
                restitution: 0.02,
            },
        }
    }

    fn stack_damping(kind: StackObjectKind) -> StackDamping {
        match kind {
            StackObjectKind::Horse | StackObjectKind::Cow => StackDamping {
                angular: 0.5,
                linear: 0.16,
            },
            StackObjectKind::Hay => StackDamping {
                angular: 0.6,
                linear: 0.18,
            },
            StackObjectKind::Chickens => StackDamping {
                angular: 0.45,
                linear: 0.12,
            },
        }
    }

    fn apply_stack_assist(world: &mut HorseStackWorld) {
        for body in &world.physics.bodies {
            let mut body = body.write().unwrap();
            if body.body_type != Physics2DBodyType::Dynamic {
                continue;
            }
            let Some(kind) = find_stack_body_kind(&world.body_registry, &body) else {
                continue;
            };
            let base = stack_damping(kind);
            if body.sleeping {
                body.linear_damping = base.linear + ASSIST_LINEAR;
                body.angular_damping = base.angular + ASSIST_ANGULAR;
                apply_assist_inertia(&mut world.body_registry, &mut body, 1.0);
                let x = body.x;
                set_stack_body_anchor(&mut world.body_registry, &body, x);
                continue;
            }
            let still = body.velocity_x.hypot(body.velocity_y) < STILL_LINEAR
                && body.angular_velocity.abs() < STILL_ANGULAR;
            let quiet = (get_stack_body_quiet(&world.body_registry, &body)
                + if still {
                    PHYSICS_STEP
                } else {
                    -PHYSICS_STEP * 3.0
                })
            .clamp(0.0, SETTLE_DELAY + SETTLE_RAMP);
            set_stack_body_quiet(&mut world.body_registry, &body, quiet);
            let settled = ((quiet - SETTLE_DELAY) / SETTLE_RAMP).clamp(0.0, 1.0) as f32;
            let carried =
                ((body.y - PASTURE_TOP_Y as f32) / STABILITY_FULL_HEIGHT).clamp(0.0, 1.0) * 0.75;
            let assist = settled.max(carried);
            body.linear_damping = base.linear + assist * ASSIST_LINEAR;
            body.angular_damping = base.angular + assist * ASSIST_ANGULAR;
            apply_assist_inertia(&mut world.body_registry, &mut body, assist);
            apply_cohesion(&mut world.body_registry, &mut body, assist, still);
        }
    }

    fn apply_cohesion(
        registry: &mut StackBodyRegistry,
        body: &mut RigidBody2D,
        assist: f32,
        still: bool,
    ) {
        if assist <= 0.0 || !still {
            delete_stack_body_anchor(registry, body);
            return;
        }
        let Some(anchor) = get_stack_body_anchor(registry, body) else {
            set_stack_body_anchor(registry, body, body.x);
            return;
        };
        let drift = anchor - body.x;
        if drift.abs() < COHESION_DEADZONE {
            return;
        }
        body.velocity_x += drift * COHESION_PULL * assist * PHYSICS_STEP as f32;
        body.velocity_x -= body.velocity_x * COHESION_GRIP * assist * PHYSICS_STEP as f32;
    }

    fn apply_assist_inertia(registry: &mut StackBodyRegistry, body: &mut RigidBody2D, assist: f32) {
        let solved = get_stack_body_inertia(registry, body).unwrap_or(body.inertia);
        set_stack_body_inertia(registry, body, solved);
        if solved <= 0.0 {
            return;
        }
        let stiffened = solved * (1.0 + assist * ASSIST_INERTIA);
        body.inertia = stiffened;
        body.inverse_inertia = 1.0 / stiffened;
    }

    fn get_fallen_below_floor_y(
        registry: &StackBodyRegistry,
        body: &RigidBody2D,
        ground: Option<&GroundProfile>,
    ) -> f64 {
        let half_height = get_stack_object_profile(get_stack_body_kind(registry, body)).half_height;
        let ground_drop = (PASTURE_TOP_Y - get_ground_y(ground, f64::from(body.x))).max(0.0);
        -half_height - ground_drop
    }

    fn is_stack_body_on_stack(
        registry: &StackBodyRegistry,
        body: &RigidBody2D,
        ground: Option<&GroundProfile>,
    ) -> bool {
        is_stack_body_within_pasture(registry, body)
            && f64::from(body.y) >= get_fallen_below_floor_y(registry, body, ground)
            && body.velocity_y.abs() <= 1.2
    }
}

#[cfg(feature = "flight-physics")]
pub use flight::*;

#[cfg(test)]
mod tests {
    use super::{GroundProfile, get_ground_y};
    use crate::physics::PASTURE_TOP_Y;

    #[test]
    fn ground_height_interpolates_and_clamps_to_the_sampled_profile() {
        let ground = GroundProfile {
            min_x: -1.0,
            step: 1.0,
            heights: vec![-0.1, 0.1, 0.3],
        };
        assert_eq!(get_ground_y(Some(&ground), -2.0), -0.1);
        assert!((get_ground_y(Some(&ground), -0.5) - 0.0).abs() < f64::EPSILON);
        assert_eq!(get_ground_y(Some(&ground), 3.0), 0.3);
        assert_eq!(get_ground_y(None, 0.0), PASTURE_TOP_Y);
    }
}

#[cfg(all(test, feature = "flight-physics"))]
mod flight_tests {
    use std::slice;

    use super::{
        STACK_CONTACT_GROUND, add_stack_object_body, create_horse_stack_world,
        does_stack_placement_overlap, get_settled_stack_height, get_stack_body_contacts,
        step_horse_stack,
    };
    use crate::physics::{StackObjectKind, get_stack_object_profile};

    #[test]
    fn world_and_stack_body_use_the_typescript_solver_tuning() {
        let mut world = create_horse_stack_world(None);
        assert_eq!(world.physics.gravity_y, -10.8);
        assert_eq!(world.physics.config.velocity_iterations, 18);
        assert_eq!(world.physics.config.position_iterations, 9);
        assert_eq!(world.physics.bodies.len(), 1);

        let body = add_stack_object_body(&mut world, StackObjectKind::Horse, 0.0, 0.4, 0.0);
        for _ in 0..180 {
            step_horse_stack(&mut world);
        }
        let body_state = body.read().unwrap();
        assert!(body_state.x.is_finite());
        assert!(body_state.y.is_finite());
        assert!(body_state.angle.is_finite());
        drop(body_state);
        assert_ne!(
            get_stack_body_contacts(&world, &body) & STACK_CONTACT_GROUND,
            0
        );
    }

    #[test]
    fn placement_overlap_uses_the_authored_collider_shapes() {
        let mut world = create_horse_stack_world(None);
        let body = add_stack_object_body(&mut world, StackObjectKind::Hay, 0.0, 0.2, 0.0);
        assert!(does_stack_placement_overlap(
            StackObjectKind::Hay,
            0.0,
            0.2,
            0.0,
            slice::from_ref(&body)
        ));
        assert!(!does_stack_placement_overlap(
            StackObjectKind::Hay,
            1.0,
            0.2,
            0.0,
            slice::from_ref(&body)
        ));
    }

    #[test]
    fn settled_height_reads_sleeping_stack_bodies() {
        let mut world = create_horse_stack_world(None);
        let body = add_stack_object_body(&mut world, StackObjectKind::Cow, 0.0, 0.2, 0.0);
        body.write().unwrap().sleeping = true;
        let expected = 0.2 + get_stack_object_profile(StackObjectKind::Cow).half_height;
        let height = get_settled_stack_height(&world, slice::from_ref(&body), None);
        assert!((height - expected).abs() < 1e-6);
    }
}
