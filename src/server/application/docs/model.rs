#[derive(Debug, Clone)]
pub struct RawGuide {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub section: String,
    pub order: i32,
    pub body: String,
}
