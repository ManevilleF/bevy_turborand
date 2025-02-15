//! A plugin to enable random number generation for the Bevy game engine.
//!
//! The plugin makes use of [`turborand`](https://docs.rs/turborand/latest/turborand/),
//! on which the implementation uses [Wyrand](https://github.com/wangyi-fudan/wyhash),
//! a simple and fast generator but **not** cryptographically secure.
//!
//! This plugin exposes a [`GlobalRng`] for use as a `Resource`, as well as
//! a [`RngComponent`] for providing rng instances at a per-entity level. By
//! exposing random number generation as a component allows for better
//! parallelisation of systems making use of PRNG, as well as making it
//! easier to enable determinism in an otherwise multi-threaded engine.
//! Relying on a single `Rng` instance for the entire application is not
//! conducive to multi-threading, and imposes far too strict ordering
//! requirements in order to ensure that each time the Rng is called and its
//! internal state is modified, that it is done so in a deterministic manner.
//!
//! By splitting one instance into components, each [`RngComponent`] only is
//! responsible for the entity it is applied to. It also prevents other
//! actions in the game from affecting the outcome of unrelated entities.
//! Also, Bevy's queries are not stable in their ordering, so each time
//! a system runs, the order by which a query iterates through selected
//! entities will be different. By providing an instance of an [`Rng`] per
//! entity, it then makes the question of stable ordering in queries moot.
//! Thus, determinism can be achieved regardless of unstable query ordering
//! and multi-threaded execution.
//!
//! # Usage
//!
//! For both [`GlobalRng`] and [`RngComponent`], both must be accessed with a
//! `mut` reference in order to ensure each system using the [`Rng`] instances
//! are not run in parallel over the entities being accessed. This should
//! also allow for better diagnostics with Bevy's ambiguous ordering tool
//! for finding systems that should be more explicitly ordered.
//!
//! On its own, the [`Rng`] is not threadsafe unless it is accessed via a `mut`
//! reference. By doing so, the `Rng` can be even more performant and not
//! have to rely on atomics (which impose considerable overhead).
//!
//! After requesting [`GlobalRng`] resource or the [`RngComponent`], one **must**
//! call `.get_mut()` in order to get the [`Rng`] instance itself. From there,
//! all [`Rng`] methods in [`turborand`](https://docs.rs/turborand/latest/turborand/)
//! are available to be used.
//!
//! [`GlobalRng`] is provided as a means to seed [`RngComponent`] with randomised
//! states, and should **not** be used as a direct source of entropy for
//! systems in general. All systems that access the [`GlobalRng`] cannot be
//! parallelised easily, so [`RngComponent`] should be used instead as much as
//! possible. On the plus side with [`GlobalRng`], only one seed needs to be
//! provided in order to have all instances be deterministic, as long as all
//! [`RngComponent`]s are created using [`GlobalRng`].
//!
//! # Example
//!
//! Basic example of setting up and using the Rng.
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_turborand::*;
//!
//! #[derive(Debug, Component)]
//! struct Player;
//!
//! fn setup_player(mut commands: Commands, mut global_rng: ResMut<GlobalRng>) {
//!     commands.spawn()
//!         .insert(Player)
//!         .insert(RngComponent::from_global(&mut global_rng));
//! }
//!
//! fn do_damage(mut q_player: Query<&RngComponent, With<Player>>) {
//!     let rng = q_player.single();
//!     println!("Player attacked for {} damage!", rng.u32(10..=20));
//! }
//!
//! fn main() {
//!     App::new()
//!         .add_plugin(RngPlugin::default())
//!         .add_startup_system(setup_player)
//!         .add_system(do_damage)
//!         .run();
//! }
//! ```
//! 
//! # How to enable Determinism
//! 
//! In order to obtain determinism for your game/app, the [`Rng`]'s must be
//! seeded. [`GlobalRng`] and [`RngPlugin`] can given a seed which then sets the
//! internal PRNG to behave deterministically. Instead of having to seed every
//! [`RngComponent`] manually, as long as the [`GlobalRng`] is seeded, then
//! [`RngComponent`] can be created directly from the global instance, cloning
//! the internal Rng to itself, which gives it a random but deterministic seed.
//! This allows for better randomised states among [`RngComponent`]s while still
//! having a deterministic app.
//! 
//! Systems also must be ordered correctly for determinism to occur. Systems
//! however do not need to be strictly ordered against every one as if some
//! linear path. Only related systems that access a given set of [`RngComponent`]s
//! need to be ordered. Ones that are unrelated can run in parallel and still
//! yield a deterministic result. So systems selecting a `Player` entity with 
//! a [`RngComponent`] should all be ordered against each other, but systems
//! selecting an `Item` entity with an [`RngComponent`] that never interacts with
//! `Player` don't need to be ordered with `Player` systems, only between
//! themselves.
//! 
//! To see an example of this, view the project's tests to see how to make
//! use of determinism for testing random systems.
//! 
//! # Caveats about Determinism
//! 
//! Any [`Rng`] method that relies on `usize` will not exhibit the same result
//! on 64-bit systems and 32-bit systems. The [`Rng`] output will be different
//! on those platforms, though it will be deterministically different. This is
//! because the output of the `WyRand` algorithm for usize on 32-bit platforms
//! is `u32` and thus is truncating the full `u64` output from the generator.
//! As such, it will not be the same value between 32-bit and 64-bit platforms.
//! 
//! Methods that are susceptible to this are `usize`, `sample`, `weighted_sample`
//! and `shuffle`.
#![warn(missing_docs, rust_2018_idioms)]

use bevy::prelude::*;
use turborand::{AtomicState, Rng, atomic_rng};

/// Module for dealing directly with [`turborand`] and its features.
///
/// # Examples
///
/// Generate a random value:
///
/// ```
/// use bevy_turborand::rng::*;
///
/// let rand = rng!();
///
/// let value = rand.bool();
/// ```
///
/// Sample a value from a list:
///
/// ```
/// use bevy_turborand::rng::*;
///
/// let rand = rng!();
///
/// let values = [1, 2, 3, 4, 5];
///
/// let value = rand.sample(&values);
/// ```
///
/// Generate a vector with random values:
///
/// ```
/// use bevy_turborand::rng::*;
/// use std::iter::repeat_with;
///
/// let rand = rng!();
///
/// let values: Vec<_> = repeat_with(|| rand.f32()).take(10).collect();
/// ```
pub mod rng {
    pub use turborand::*;
}

/// A Global [`Rng`] instance, meant for use as a Resource. Gets
/// created automatically with [`RngPlugin`], or can be created
/// and added manually.
#[derive(Debug, Deref, DerefMut)]
pub struct GlobalRng(Rng<AtomicState>);

unsafe impl Sync for GlobalRng {}

impl GlobalRng {
    /// Create a new [`GlobalRng`] instance with an optional seed value.
    /// Uses a randomised seed if `None` is provided.
    #[inline]
    #[must_use]
    pub fn new(seed: Option<u64>) -> Self {
        match seed {
            Some(s) => Self::with_seed(s),
            None => Self::randomized(),
        }
    }

    /// Create a new [`GlobalRng`] instance with a seed value.
    #[inline]
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        Self(atomic_rng!(seed))
    }

    /// Create a new [`GlobalRng`] instance with a randomized seed value.
    #[inline]
    #[must_use]
    pub fn randomized() -> Self {
        Self(atomic_rng!())
    }

    /// Returns the internal [`Rng<CellState>`] reference.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self) -> &mut Rng<AtomicState> {
        &mut self.0
    }
}

impl Default for GlobalRng {
    /// Creates a default [`GlobalRng`] instance. The instance will
    /// be initialised with a randomised seed, so this is **not**
    /// deterministic.
    fn default() -> Self {
        Self::new(None)
    }
}

/// A [`Rng`] component that wraps a random number generator.
#[derive(Debug, Deref, DerefMut, Component)]
pub struct RngComponent(Rng<AtomicState>);

unsafe impl Sync for RngComponent {}

impl RngComponent {
    /// Create a new [`RngComponent`] instance with an optional seed value.
    /// Uses a randomised seed if `None` is provided.
    #[inline]
    #[must_use]
    pub fn new(seed: Option<u64>) -> Self {
        match seed {
            Some(s) => Self::with_seed(s),
            None => Self::randomized(),
        }
    }

    /// Create a new [`RngComponent`] instance with a seed value.
    #[inline]
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        Self(atomic_rng!(seed))
    }

    /// Create a new [`RngComponent`] instance with a randomized seed value.
    #[inline]
    #[must_use]
    pub fn randomized() -> Self {
        Self(atomic_rng!())
    }

    /// Creates a new [`RngComponent`] instance by cloning from an [`Rng<CellState>`].
    #[inline]
    #[must_use]
    pub fn from_rng(rng: &Rng<AtomicState>) -> Self {
        Self(rng.clone())
    }

    /// Creates a new [`RngComponent`] directly from a [`GlobalRng`], by cloning the
    /// internal [`Rng<CellState>`] instance.
    #[inline]
    #[must_use]
    pub fn from_global(rng: &mut GlobalRng) -> Self {
        Self::from_rng(rng.get_mut())
    }

    /// Returns the internal [`Rng<CellState>`] reference.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self) -> &mut Rng<AtomicState> {
        &mut self.0
    }
}

impl Default for RngComponent {
    /// Creates a default [`RngComponent`] instance. The instance will
    /// be initialised with a randomised seed, so this is **not**
    /// deterministic.
    fn default() -> Self {
        Self::new(None)
    }
}

/// A [`Plugin`] for initialising a [`GlobalRng`] into a Bevy `App`.
#[derive(Debug)]
pub struct RngPlugin(Option<u64>);

impl RngPlugin {
    /// Create a new [`RngPlugin`] instance with a seed value.
    #[inline]
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(Some(seed))
    }

    /// Create a new [`RngPlugin`] instance with a randomized seed.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(None)
    }
}

impl Default for RngPlugin {
    /// Creates a default [`RngPlugin`] instance. The instance will
    /// be initialised with a randomised seed, so this is **not**
    /// deterministic.
    fn default() -> Self {
        Self::empty()
    }
}

impl Plugin for RngPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalRng::new(self.0));
    }
}
