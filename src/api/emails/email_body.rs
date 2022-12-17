pub enum EmailBody {
    Text(String),
    Html { content: String, fallback: String },
}
