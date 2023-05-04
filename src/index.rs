use std::env::current_dir;

use axum::{response::{Html}, Form};
use chrono::{TimeZone, Local};
use liquid::ParserBuilder;
use serde::{Deserialize};
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

#[derive(Deserialize, Debug)]
pub struct Event {
    name: String,
    date: String,
    location: String,
    teacher: String,
    prefects: String,
    info: String,
}

pub async fn root_form (Form(Event {name, date, location, teacher, prefects, info}): Form<Event>) -> Html<String> {
    let date = Local::now().timezone().datetime_from_str(&date, "%Y-%m-%dT%H:%M").expect("error getting date/time");

    info!(?name, ?date, ?location, ?teacher, ?prefects, ?info, "Got form response");

    root().await
}