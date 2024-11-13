use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub logs: bool,
    pub colored_logs: bool,
    pub writeable_path: String,
    /// Should lb do background work like keep search indexes up to date?
    pub background_work: bool,
}

// todo: we added background work as a flag to speed up test execution in debug mode
// turn background work back to true in test_utils to see the slow test
// the slow test primarily does a large amount of allocations due to ownership model
// of treelike. In a universe where these operations could be expressed as iterators
// we would be able to vastly cut down on allocations and eliminate this complexity
//
// another nice aspect of background work is that it is a workaround for CLI's lack
// of graceful shutdown. Ideally, both of these situations will be handled differently.
