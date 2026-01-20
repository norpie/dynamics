/// Resource type for managing async state in a type-safe, explicit way.
///
/// Inspired by Elm's RemoteData pattern, this enum represents the four states
/// of an asynchronous operation:
/// - NotAsked: Initial state, no request made yet
/// - Loading: Request in progress
/// - Success: Request completed successfully with data
/// - Failure: Request failed with error
///
/// This eliminates the need for separate `loading: bool` and `data: Option<T>`
/// fields, making async state management cleaner and less error-prone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resource<T, E = String> {
    /// No request has been made yet (initial state)
    NotAsked,

    /// Request is in progress
    Loading,

    /// Request succeeded with data
    Success(T),

    /// Request failed with error
    Failure(E),
}

impl<T, E> Resource<T, E> {
    /// Create a Resource from a Result
    pub fn from_result(result: Result<T, E>) -> Self {
        match result {
            Ok(data) => Resource::Success(data),
            Err(e) => Resource::Failure(e),
        }
    }

    /// Check if the resource is currently loading
    pub fn is_loading(&self) -> bool {
        matches!(self, Resource::Loading)
    }

    /// Check if the resource has succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, Resource::Success(_))
    }

    /// Check if the resource has failed
    pub fn is_failure(&self) -> bool {
        matches!(self, Resource::Failure(_))
    }

    /// Check if the resource has not been asked for yet
    pub fn is_not_asked(&self) -> bool {
        matches!(self, Resource::NotAsked)
    }

    /// Get the data if successful, otherwise return default
    pub fn unwrap_or(&self, default: T) -> T
    where
        T: Clone,
    {
        match self {
            Resource::Success(data) => data.clone(),
            _ => default,
        }
    }

    /// Get a reference to the data if successful
    pub fn as_ref(&self) -> Resource<&T, &E> {
        match self {
            Resource::NotAsked => Resource::NotAsked,
            Resource::Loading => Resource::Loading,
            Resource::Success(data) => Resource::Success(data),
            Resource::Failure(e) => Resource::Failure(e),
        }
    }

    /// Map the success value to a new type
    pub fn map<U, F>(self, f: F) -> Resource<U, E>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Resource::NotAsked => Resource::NotAsked,
            Resource::Loading => Resource::Loading,
            Resource::Success(data) => Resource::Success(f(data)),
            Resource::Failure(e) => Resource::Failure(e),
        }
    }

    /// Map the error value to a new type
    pub fn map_err<F, G>(self, f: F) -> Resource<T, G>
    where
        F: FnOnce(E) -> G,
    {
        match self {
            Resource::NotAsked => Resource::NotAsked,
            Resource::Loading => Resource::Loading,
            Resource::Success(data) => Resource::Success(data),
            Resource::Failure(e) => Resource::Failure(f(e)),
        }
    }

    /// Convert to an Option, discarding error and loading states
    pub fn ok(self) -> Option<T> {
        match self {
            Resource::Success(data) => Some(data),
            _ => None,
        }
    }

    /// Convert to a Result, treating NotAsked and Loading as None
    pub fn to_option(&self) -> Option<&T> {
        match self {
            Resource::Success(data) => Some(data),
            _ => None,
        }
    }
}

impl<T, E> Default for Resource<T, E> {
    fn default() -> Self {
        Resource::NotAsked
    }
}

// Implement From<Result<T, E>> for convenience
impl<T, E> From<Result<T, E>> for Resource<T, E> {
    fn from(result: Result<T, E>) -> Self {
        Resource::from_result(result)
    }
}
