// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Path and URI models.

#[allow(clippy::module_inception)]
mod path;
mod path_component;
mod path_components;
mod path_semantics;
mod relative_path;

pub use path::Path;
pub use path_component::PathComponent;
pub use path_components::PathComponents;
pub use path_semantics::PathSemantics;
pub use relative_path::RelativePath;
