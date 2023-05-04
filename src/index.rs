use std::env::current_dir;

use axum::response::Html;
use liquid::ParserBuilder;
use tokio::fs::read_to_string;

pub async fn root () -> Html<String> {
    let liquid = read_to_string("www/templates/index.liquid").await.unwrap();
    let template = ParserBuilder::with_stdlib()
        .build().unwrap()
        .parse(&liquid).unwrap();

    let cd = current_dir().map(|cd| cd.to_str().map(|x| x.to_string())).unwrap_or(Some("failed to get cd".into())).unwrap();
    let globals = liquid::object!({
        "cd": cd
    });

    let output = template.render(&globals).unwrap();

    Html(output.to_string())
}