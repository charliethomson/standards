//! MCP over streamable HTTP at `/mcp` (L4, L7, L30).
//!
//! TODO(STD-16): mount rmcp's `StreamableHttpService` here, stateless, with
//! `state.allowed_hosts` as its Host allowlist (see the STD-13 findings). This module is
//! STD-16's alone; `app()` already calls [`mount`], so nothing else needs editing.

use poem::Route;

use crate::state::AppState;

/// Add the MCP endpoint to `route`. A no-op until STD-16.
#[must_use]
pub fn mount(route: Route, _state: &AppState) -> Route {
    route
}
