use handlebars::Handlebars;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/templates"]
#[include = "*.hbs"]
struct TemplateAssets;

/// Creates a handlebars instance with embedded templates.
pub fn create_templates<'reg>() -> anyhow::Result<Handlebars<'reg>> {
    let mut handlebars = Handlebars::new();
    handlebars.register_embed_templates_with_extension::<TemplateAssets>(".hbs")?;
    Ok(handlebars)
}
