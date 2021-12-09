use anyhow::{Context, Result};
use clap::Parser;
use percent_encoding::percent_decode_str;
use reqwest::blocking::Client;
use select::{
    document::Document,
    predicate::{Attr, Name, Predicate},
};
use serde::Deserialize;

use std::{
    env::temp_dir,
    fmt::Display,
    fs::File,
    io::{stdin, Write},
    path::{Path, PathBuf},
};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct City {
    id: String,
    label: String,
    value: String,
    custom: String,
}

impl Display for City {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = percent_decode_str(&self.label)
            .decode_utf8_lossy()
            .replace('+', " ");

        write!(f, "{}", name)
    }
}

fn search_cities(query: &str) -> Result<Vec<City>> {
    let url = "https://tempo.cptec.inpe.br/autocomplete";
    let params = [("term", query)];

    let client = Client::new();
    let response = client.get(url).query(&params).send()?;

    let json = response.text()?;
    let cities: Vec<City> = serde_json::from_str(&json)?;

    Ok(cities)
}

fn select_city_prompt(cities: &[City]) -> Result<&City> {
    for (i, city) in cities.iter().enumerate() {
        println!("[{:2}] {}", i, city);
    }

    println!("\nDigite o número da cidade desejada: ");

    let mut input_buffer = String::new();
    stdin().read_line(&mut input_buffer)?;

    let index = input_buffer.trim().parse::<usize>()?;

    Ok(&cities[index])
}

fn forecast_url(city: &City) -> String {
    format!("https://tempo.cptec.inpe.br/{}", city.custom)
}

fn fetch_forecast_page(city: &City) -> Result<String> {
    let url = forecast_url(city);

    let client = Client::new();
    let response = client.get(url).send()?;

    Ok(response.text()?)
}

fn scrape_meteogram_url(page_contents: &str) -> Option<String> {
    let doc = Document::from(page_contents);

    let selector = Name("div")
        .and(Attr("id", "meteograma"))
        .descendant(Name("img"));

    let img = doc.find(selector).next();

    img.and_then(|node| node.attr("src"))
        .map(|url| url.to_owned())
}

fn fetch_meteogram(city: &City) -> Result<Vec<u8>> {
    let page_contents = fetch_forecast_page(city)?;
    let url = scrape_meteogram_url(&page_contents).context("Could not find meteogram URL")?;

    let response = Client::new().get(url).send()?;
    Ok(response.bytes()?.to_vec())
}

fn save_meteogram(bytes: &[u8], path: &Path) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    Ok(())
}

fn show_meteogram(bytes: &[u8]) -> Result<()> {
    let temp_path = temp_dir().join("meteo.png");
    save_meteogram(bytes, &temp_path)?;
    open::that(&temp_path)?;
    Ok(())
}

#[derive(clap::Parser)]
struct Args {
    query: String,

    #[clap(short)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let cities = search_cities(&args.query)?;
    let selected_city = select_city_prompt(&cities)?;
    let meteogram = fetch_meteogram(selected_city)?;

    match args.output {
        Some(path) => save_meteogram(&meteogram, &path),
        None => show_meteogram(&meteogram),
    }
}
