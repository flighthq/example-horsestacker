//! One shared read of the native reduce-motion preference, corresponding to
//! `reducedMotion.ts`.
//!
//! The browser queries a live `MediaQueryList`. The native host owns the equivalent setting and
//! may change it while the application is running, so this remains a live atomic read rather than
//! a startup snapshot.

use std::sync::atomic::{AtomicBool, Ordering};

static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

pub fn prefers_reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Relaxed)
}

pub fn set_prefers_reduced_motion(reduced: bool) {
    REDUCED_MOTION.store(reduced, Ordering::Relaxed);
}
