pub const ID: &str = "ghost";

#[derive(Debug)]
pub struct GhostPayload {
    pub title: String,
    pub lexical: Option<String>,
    pub tags: Vec<String>,
    pub existing_id: Option<String>,
}
