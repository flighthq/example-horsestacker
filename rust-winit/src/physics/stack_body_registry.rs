//! Per-body side tables, mirroring `stackBodyRegistry.ts` without extending Flight's body type.

use std::collections::HashMap;

use flight_sdk::RigidBody2D;

use super::StackObjectKind;

#[derive(Debug, Default)]
pub struct StackBodyRegistry {
    kinds: HashMap<i32, StackObjectKind>,
    quiet: HashMap<i32, f64>,
    inertia: HashMap<i32, f32>,
    anchors: HashMap<i32, f32>,
}

/// Defaults to `Horse` for a body this registry never saw, matching the largest TS profile.
pub fn get_stack_body_kind(registry: &StackBodyRegistry, body: &RigidBody2D) -> StackObjectKind {
    find_stack_body_kind(registry, body).unwrap_or(StackObjectKind::Horse)
}

/// The undefaulted lookup used to distinguish stack pieces from the pasture body.
pub fn find_stack_body_kind(
    registry: &StackBodyRegistry,
    body: &RigidBody2D,
) -> Option<StackObjectKind> {
    registry.kinds.get(&body.index).copied()
}

pub fn set_stack_body_kind(
    registry: &mut StackBodyRegistry,
    body: &RigidBody2D,
    kind: StackObjectKind,
) {
    debug_assert!(
        body.index >= 0,
        "stack bodies are registered after insertion"
    );
    registry.kinds.insert(body.index, kind);
}

pub fn get_stack_body_quiet(registry: &StackBodyRegistry, body: &RigidBody2D) -> f64 {
    registry.quiet.get(&body.index).copied().unwrap_or(0.0)
}

pub fn set_stack_body_quiet(registry: &mut StackBodyRegistry, body: &RigidBody2D, seconds: f64) {
    registry.quiet.insert(body.index, seconds);
}

pub fn get_stack_body_inertia(registry: &StackBodyRegistry, body: &RigidBody2D) -> Option<f32> {
    registry.inertia.get(&body.index).copied()
}

pub fn set_stack_body_inertia(registry: &mut StackBodyRegistry, body: &RigidBody2D, inertia: f32) {
    registry.inertia.insert(body.index, inertia);
}

pub fn get_stack_body_anchor(registry: &StackBodyRegistry, body: &RigidBody2D) -> Option<f32> {
    registry.anchors.get(&body.index).copied()
}

pub fn set_stack_body_anchor(registry: &mut StackBodyRegistry, body: &RigidBody2D, x: f32) {
    registry.anchors.insert(body.index, x);
}

pub fn delete_stack_body_anchor(registry: &mut StackBodyRegistry, body: &RigidBody2D) {
    registry.anchors.remove(&body.index);
}

pub fn delete_stack_body(registry: &mut StackBodyRegistry, body: &RigidBody2D) {
    registry.kinds.remove(&body.index);
    registry.quiet.remove(&body.index);
    registry.inertia.remove(&body.index);
    registry.anchors.remove(&body.index);
}
