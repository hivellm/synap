//! MCP Authentication Context Storage
//!
//! Provides thread-local storage for AuthContext during MCP request processing

use super::AuthContext;
use std::cell::RefCell;

thread_local! {
    /// Thread-local storage for AuthContext during MCP request processing
    static MCP_AUTH_CONTEXT: RefCell<Option<AuthContext>> = RefCell::new(None);
}

/// Set the authentication context for the current thread
pub fn set_auth_context(ctx: AuthContext) {
    MCP_AUTH_CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(ctx);
    });
}

/// Get the authentication context for the current thread
pub fn get_auth_context() -> Option<AuthContext> {
    MCP_AUTH_CONTEXT.with(|cell| cell.borrow().clone())
}

/// Clear the authentication context for the current thread
pub fn clear_auth_context() {
    MCP_AUTH_CONTEXT.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Execute a function with the given authentication context
pub fn with_auth_context<F, R>(ctx: AuthContext, f: F) -> R
where
    F: FnOnce() -> R,
{
    set_auth_context(ctx);
    let result = f();
    clear_auth_context();
    result
}
