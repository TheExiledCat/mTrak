pub const TITLE: &str = "mTrak";
pub const COMMAND: &str = "mtrak";

#[derive(Clone)]
pub struct Config {}
impl Default for Config {
    fn default() -> Self {
        return Self {};
    }
}
