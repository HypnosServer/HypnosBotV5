use std::{fs::{read_to_string, File}, io::{BufReader, Read}, ops::Deref};

use flate2::bufread::GzDecoder;
use futures::{future, Stream, StreamExt};
use poise::{command, CreateReply};
use valence_nbt::{from_binary, Compound, List, Value};

use crate::{commands::prelude::*, Scoreboards};




async fn score_autocomplete_board<'a>(
    ctx: Context<'a>,
    partial: &'a str
) -> impl Stream<Item = String> + 'a {
    let data = ctx.serenity_context().data.read().await;
    let scoreboards = data.get::<Scoreboards>()
        .expect("Scoreboards not found in data");
    futures::stream::iter(
        scoreboards.clone().into_iter()
            .filter(move |board| board.starts_with(partial))
            .map(String::from)
    )
}

#[command(slash_command, prefix_command)]
pub async fn score(
    ctx: Context<'_>,
    #[description = "The board to display scores for"]
    #[autocomplete = "score_autocomplete_board"]
    board: String,
) -> Result<(), Error> {
    Ok(())
}