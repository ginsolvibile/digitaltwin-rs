use std::collections::HashMap;

pub type ActorStateType = dyn ActorState + Send + Sync + 'static;

pub trait ActorState {
    /// Handle the change of an input slot
    fn input_change(&self, slot: &str, value: f32) -> Box<ActorStateType>;
    /// Execute a command
    fn execute(&self, command: &str, input: serde_json::Value) -> Box<ActorStateType>;

    // Helper functions
    fn as_any(&self) -> &dyn std::any::Any;
    fn type_name(&self) -> String;
    fn state(&self) -> String;
}

impl std::fmt::Debug for Box<ActorStateType> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.type_name(), self.state())
    }
}

/// Factory trait for creating actors. Each Actor type must implement this trait
/// to provide a default instance and a way to create instances with parameters.
pub trait ActorFactory {
    fn create_default() -> (Box<ActorStateType>, Vec<&'static str>);
    fn create_with_params(params: serde_json::Value) -> (Box<ActorStateType>, Vec<&'static str>);
}

/// State behavior trait for providing the input and command handler dispatch maps.
pub trait StateBehavior {
    /// The actor type that uses this state
    type Actor;

    /// Create the update dispatch map
    fn create_dispatch_map() -> DispatchMap<Self::Actor>;
    /// Create the command dispatch map
    fn create_command_map() -> CommandMap<Self::Actor>;

    fn state_name() -> String;
}

/// The dispatch map associates input slots (strings) with their handlers
pub type DispatchMap<A> = HashMap<&'static str, fn(&A, f32) -> Box<ActorStateType>>;
/// The command map associates commands (strings) with their handlers
pub type CommandMap<A> = HashMap<&'static str, fn(&A, serde_json::Value) -> Box<ActorStateType>>;
