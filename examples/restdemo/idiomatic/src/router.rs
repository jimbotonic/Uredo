//! Routing as a `match`, decided at compile time.

/// Every route this service has.
#[derive(Debug, PartialEq)]
pub enum Route {
    Health,
    ListItems,
    CreateItem,
    GetItem(u64),
    PatchItem(u64),
    DeleteItem(u64),
}

/// Resolve a method and path without allocating.
pub fn resolve(method: &str, path: &str) -> Option<Route> {
    let trimmed = path.trim_end_matches('/');
    let mut parts = trimmed.split('/').filter(|s| !s.is_empty());
    let first = parts.next();
    let second = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (method, first, second) {
        ("GET", Some("health"), None) => Some(Route::Health),
        ("GET", Some("items"), None) => Some(Route::ListItems),
        ("POST", Some("items"), None) => Some(Route::CreateItem),
        ("GET", Some("items"), Some(id)) => id.parse().ok().map(Route::GetItem),
        ("PATCH", Some("items"), Some(id)) => id.parse().ok().map(Route::PatchItem),
        ("DELETE", Some("items"), Some(id)) => id.parse().ok().map(Route::DeleteItem),
        _ => None,
    }
}
