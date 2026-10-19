//! The rendered HTML site: `/` and `/d/{id}` (L6, L25).
//!
//! TODO(STD-17): add the index and doc pages here (no inline JS, CSP `script-src 'self'`).
//! This module is STD-17's alone; `app()` already calls [`mount`], so nothing else needs
//! editing. Wrap the pages in `caching::Immutable::new(state.etag.clone())` like the REST
//! mirror.

use poem::Route;

use crate::state::AppState;

/// Add the HTML routes to `route`. A no-op until STD-17.
#[must_use]
pub fn mount(route: Route, _state: &AppState) -> Route {
    route
}
